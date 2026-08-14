//! Demo driver for the airline management system.

use flight_management_system::adapters::gateway::MockGateway;
use flight_management_system::time::{Date, Time};
use flight_management_system::{
    Aircraft, AirlineManagementSystem, AmsError, Baggage, CrewMember, CrewRole, FlightSpec,
    PaymentMethod, PaymentStatus, SeatStatus, User, UserRole,
};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ams = AirlineManagementSystem::with_gateway(Box::new(MockGateway::default()));
    let admin = User::new(100, "Ops Admin", UserRole::Admin);
    let staff = User::new(101, "Duty Staff", UserRole::Staff);

    let a320 = Arc::new(Aircraft {
        tail_number: "VT-ABC".into(),
        model: "Airbus A320".into(),
        total_seats: 36,
    });
    let morning = ams.schedule_flight(
        &admin,
        a320,
        FlightSpec {
            source: "DEL".into(),
            destination: "BOM".into(),
            date: Date::new(2026, 5, 10),
            departure: Time::new(9, 30),
            arrival: Time::new(11, 45),
        },
        6_500.0,
    )?;
    ams.assign_crew(
        &admin,
        &morning.flight_number,
        vec![
            CrewMember {
                id: 1,
                name: "Capt. Sharma".into(),
                role: CrewRole::Captain,
            },
            CrewMember {
                id: 2,
                name: "F/O Iyer".into(),
                role: CrewRole::FirstOfficer,
            },
            CrewMember {
                id: 3,
                name: "Priya (CC)".into(),
                role: CrewRole::CabinCrew,
            },
        ],
    )?;

    let a321 = Arc::new(Aircraft {
        tail_number: "VT-XYZ".into(),
        model: "Airbus A321".into(),
        total_seats: 36,
    });
    let evening = ams.schedule_flight(
        &admin,
        a321,
        FlightSpec {
            source: "DEL".into(),
            destination: "BOM".into(),
            date: Date::new(2026, 5, 10),
            departure: Time::new(17, 0),
            arrival: Time::new(19, 15),
        },
        7_200.0,
    )?;

    let hits = ams
        .flight_search
        .search("DEL", "BOM", Date::new(2026, 5, 10));
    println!("=== DEL → BOM on 2026-05-10 ===");
    for flight in &hits {
        println!(
            "  {}  {} → {}  dep {}  {}  avail {:>2}  from ₹{:.0}",
            flight.flight_number,
            flight.source,
            flight.destination,
            flight.departure,
            flight.aircraft_model,
            flight.available_seats,
            flight.min_fare,
        );
    }

    let asha = ams.register_passenger("Asha Nair", "asha@example.com", "+91-98100-12345");
    let asha_user = ams.user(asha.id).ok_or("passenger not found")?;
    let (booking, payment) = ams.bookings.book(
        &asha_user,
        &morning.flight_number,
        "1A",
        PaymentMethod::Card,
    )?;
    println!("\n=== Booking {} ===", booking.booking_number);
    println!(
        "  {} seat {} ({:?})  ₹{:.2}  payment {} [{}]",
        booking.flight_number,
        booking.seat_number,
        booking.seat_type,
        booking.price,
        payment.payment_id,
        status_label(&payment.status),
    );

    let bob = ams.register_passenger("Bob Rao", "bob@example.com", "+91-98200-67890");
    let bob_user = ams.user(bob.id).ok_or("passenger not found")?;
    match ams
        .bookings
        .book(&bob_user, &morning.flight_number, "1A", PaymentMethod::Upi)
    {
        Err(AmsError::SeatNotAvailable(message)) => println!("\n  Correctly rejected: {message}"),
        other => panic!("expected SeatNotAvailable, got {other:?}"),
    }

    ams.bookings.set_baggage(
        booking.booking_number,
        Some(Baggage {
            pieces: 1,
            total_weight_kg: 18.5,
            checked: true,
        }),
        &asha_user,
    )?;

    let changed = ams.bookings.change_flight(
        booking.booking_number,
        &evening.flight_number,
        "2A",
        PaymentMethod::Wallet,
        &asha_user,
    )?;
    println!(
        "\n=== Changed to {} seat {} ===",
        changed.booking.flight_number, changed.booking.seat_number
    );
    let settle = if changed.fare_difference >= 0.0 {
        format!("charged ₹{:.2}", changed.fare_difference)
    } else {
        format!("refunded ₹{:.2}", -changed.fare_difference)
    };
    println!(
        "  new price ₹{:.2} (fare difference → {settle})",
        changed.booking.price
    );

    let cancelled = ams.bookings.cancel(booking.booking_number, &staff)?;
    println!("\n=== Cancelled {} by staff ===", cancelled.booking_number);
    println!("  refunded ₹{:.2}", cancelled.refunded);
    let evening_seats = evening.seat_map();
    let seat = evening_seats
        .iter()
        .find(|seat| seat.number == "2A")
        .ok_or("seat 2A missing")?;
    println!("  seat 2A now {}", seat_label(&seat.status));
    println!(
        "  evening flight now has {} seats available",
        evening.available_seats()
    );

    println!("\n=== Crew manifest on {} ===", morning.flight_number);
    for crew in morning.crew_manifest() {
        println!("  {} — {:?}", crew.name, crew.role);
    }
    Ok(())
}

fn status_label(status: &PaymentStatus) -> &'static str {
    match status {
        PaymentStatus::Completed => "Completed",
        PaymentStatus::PartiallyRefunded => "Partially refunded",
        PaymentStatus::Refunded => "Refunded",
        PaymentStatus::Pending => "Pending",
        PaymentStatus::Failed => "Failed",
    }
}

fn seat_label(status: &SeatStatus) -> &'static str {
    match status {
        SeatStatus::Available => "Available",
        SeatStatus::Reserved => "Reserved",
        SeatStatus::Booked => "Booked",
    }
}
