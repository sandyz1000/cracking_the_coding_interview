//! Demo driver for the car rental system.

use car_rental_system::adapters::gateway::MockGateway;
use car_rental_system::domain::accounts::UserRole;
use car_rental_system::domain::payments::PaymentStatus;
use car_rental_system::time::Date;
use car_rental_system::{
    Address, CarError, CarRentalSystem, PaymentMethod, ReservationSpec, User, Vehicle, VehicleSpec,
    VehicleStatus,
};

fn payment_label(status: PaymentStatus) -> &'static str {
    match status {
        PaymentStatus::Completed => "Completed",
        PaymentStatus::PartiallyRefunded => "Partially refunded",
        PaymentStatus::Refunded => "Refunded",
        PaymentStatus::Pending => "Pending",
        PaymentStatus::Failed => "Failed",
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let system = CarRentalSystem::with_gateway(Box::new(MockGateway::default()));
    let manager = User::new(100, "Ops Manager", UserRole::Manager);
    let receptionist = User::new(101, "Rita", UserRole::Receptionist);

    system.add_new_location(
        "NYC Branch".to_string(),
        Address {
            street: "123 Main St".to_string(),
            city: "New York".to_string(),
            state: "NY".to_string(),
            zip_code: "10001".to_string(),
            country: "USA".to_string(),
        },
    );

    let fleet = [
        ("VT-1001", "Toyota", "Camry", 2022, 50_00),
        ("VT-1002", "Toyota", "Corolla", 2021, 45_00),
        ("VT-1003", "Honda", "Civic", 2023, 55_00),
        ("VT-1004", "Ford", "Mustang", 2020, 90_00),
    ];
    for (barcode, make, model, year, price) in fleet {
        system.add_vehicle(Vehicle::new(VehicleSpec {
            barcode: barcode.to_string(),
            license_number: format!("{barcode}-LIC"),
            stock_number: format!("STK-{barcode}"),
            capacity: 5,
            make: make.to_string(),
            model: model.to_string(),
            year,
            mileage: 20_000,
            price_per_day: price,
        }));
    }

    // Out of service: it must never appear in search results.
    system.set_vehicle_status("VT-1003", VehicleStatus::BeingServiced, &manager)?;

    println!("=== Fleet ===");
    for barcode in ["VT-1001", "VT-1002", "VT-1003", "VT-1004"] {
        let v = system.vehicle_snapshot(barcode).ok_or("vehicle missing")?;
        println!(
            "  {} {} {} ({} · {} · {} km) — ₹{}/day, {:?}",
            v.make,
            v.model,
            v.year,
            v.license_number,
            v.stock_number,
            v.mileage,
            v.price_per_day / 100,
            v.status
        );
    }

    let start = Date::new(2026, 5, 10);
    let end = Date::new(2026, 5, 13);

    let hits = system
        .search
        .search(Some("Toyota"), None, None, Some(70_00), start, end);
    println!("\n=== Available Toyotas under ₹700/day ===");
    for v in &hits {
        println!(
            "  {} {} {} — ₹{}/day",
            v.make,
            v.model,
            v.barcode,
            v.price_per_day / 100
        );
    }

    let alice = system.register_customer("Alice", "alice@example.com", "555-0100", "DL-001");
    let alice_user = system.user(alice.id).ok_or("alice user missing")?;

    // Only a manager may remove a vehicle.
    system.add_vehicle(Vehicle::new(VehicleSpec {
        barcode: "VT-1099".to_string(),
        license_number: "VT-1099-LIC".to_string(),
        stock_number: "STK-1099".to_string(),
        capacity: 2,
        make: "VW".to_string(),
        model: "Beetle".to_string(),
        year: 1995,
        mileage: 250_000,
        price_per_day: 2_000,
    }));
    match system.remove_vehicle("VT-1099", &alice_user) {
        Err(CarError::PermissionDenied) => {
            println!("  Customer cannot remove vehicles (permission denied)")
        }
        other => return Err(format!("expected PermissionDenied, got {other:?}").into()),
    }
    system.remove_vehicle("VT-1099", &manager)?;
    println!("  Manager removed the scrap vehicle");
    let (reservation, payment) = system.reservations.book(
        alice.id,
        "VT-1001",
        ReservationSpec {
            start,
            end,
            pickup: "NYC Branch",
            dropoff: "NYC Branch",
        },
        PaymentMethod::CreditCard,
    )?;
    println!("\n=== Reservation {} ===", reservation.reservation_number);
    println!(
        "  {} nights ({} → {}) · ₹{} total · payment {} [{}]",
        reservation.nights(),
        reservation.start_date,
        reservation.end_date,
        reservation.total_price / 100,
        payment.payment_id,
        payment_label(payment.status)
    );

    // Same car, same dates: must be rejected.
    let bob = system.register_customer("Bob", "bob@example.com", "555-0101", "DL-002");
    match system.reservations.book(
        bob.id,
        "VT-1001",
        ReservationSpec {
            start,
            end,
            pickup: "NYC Branch",
            dropoff: "NYC Branch",
        },
        PaymentMethod::PayPal,
    ) {
        Err(CarError::VehicleNotAvailable(message)) => {
            println!("\n  Correctly rejected: {message}")
        }
        other => return Err(format!("expected VehicleNotAvailable, got {other:?}").into()),
    }

    let (later_res, _) = system.reservations.book(
        bob.id,
        "VT-1002",
        ReservationSpec {
            start: Date::new(2026, 5, 20),
            end: Date::new(2026, 5, 22),
            pickup: "NYC Branch",
            dropoff: "NYC Branch",
        },
        PaymentMethod::PayPal,
    )?;

    let bob_user = system.user(bob.id).ok_or("bob user missing")?;
    // Staff may act on the customer's behalf.
    system
        .reservations
        .start_rental(&later_res.reservation_number, &receptionist)?;
    println!(
        "\n  {} → Loaned (started by receptionist {})",
        later_res.reservation_number, receptionist.name
    );
    system
        .reservations
        .complete_rental(&later_res.reservation_number, &bob_user)?;
    println!(
        "  {} → Completed, car back to Available",
        later_res.reservation_number
    );

    let extended_end = Date::new(2026, 5, 15);
    let delta = system.reservations.modify(
        &reservation.reservation_number,
        &alice_user,
        start,
        extended_end,
        PaymentMethod::CreditCard,
    )?;
    println!("\n=== Modified {} ===", reservation.reservation_number);
    println!(
        "  extended to {} → extra charged ₹{}",
        extended_end,
        delta / 100
    );

    let refunded = system
        .reservations
        .cancel(&reservation.reservation_number, &alice_user)?;
    println!("\n  Cancelled → refunded ₹{}", refunded / 100);

    let free_again = system
        .search
        .search(Some("Toyota"), None, None, Some(70_00), start, end);
    println!("\n=== Available Toyotas after Alice cancels ===");
    for v in &free_again {
        println!(
            "  {} {} {} — ₹{}/day",
            v.make,
            v.model,
            v.barcode,
            v.price_per_day / 100
        );
    }

    match system.reservations.book(
        bob.id,
        "VT-1004",
        ReservationSpec {
            start: Date::new(2026, 6, 1),
            end: Date::new(2026, 6, 3),
            pickup: "NYC Branch",
            dropoff: "NYC Branch",
        },
        PaymentMethod::PayPal,
    ) {
        Ok((booking, _)) => println!(
            "\n  Receptionist booked {} for {} ({} nights)",
            booking.vehicle_barcode,
            bob.name,
            booking.nights()
        ),
        Err(e) => return Err(e.into()),
    }

    println!("\nCar Rental System initialized");
    Ok(())
}
