/* 
Designing a Car Rental System

### Requirements

- The car rental system should allow customers to browse and reserve available cars for specific dates.
- Each car should have details such as make, model, year, license plate number, and rental price per day.
- Customers should be able to search for cars based on various criteria, such as car type, price range, and availability.
- The system should handle reservations, including creating, modifying, and canceling reservations.
- The system should keep track of the availability of cars and update their status accordingly.
- The system should handle customer information, including name, contact details, and driver's license information.
- The system should handle payment processing for reservations.
- The system should be able to handle concurrent reservations and ensure data consistency.

Classes, Interfaces and Enumerations

- The Car class represents a car in the rental system, with properties such as make, model, year, license plate number, 
rental price per day, and availability status.
- The Customer class represents a customer, with properties like name, contact information, and driver's license number.
- The Reservation class represents a reservation made by a customer for a specific car and date range. It includes properties 
such as reservation ID, customer, car, start date, end date, and total price.
- The PaymentProcessor interface defines the contract for payment processing, and the CreditCardPaymentProcessor and 
PayPalPaymentProcessor classes are concrete implementations of the payment processor.
- The RentalSystem class is the core of the car rental system and follows the Singleton pattern to ensure a single instance 
of the rental system.
- The RentalSystem class uses concurrent data structures (ConcurrentHashMap) to handle concurrent access to cars and reservations.
- The RentalSystem class provides methods for adding and removing cars, searching for available cars based on criteria, making 
reservations, canceling reservations, and processing payments.
- The CarRentalSystem class serves as the entry point of the application and demonstrates the usage of the car rental system.

*/

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use chrono::NaiveDate;

// Enums for various statuses and types
// The unused variants are spec vocabulary: a real fleet/ops flow transitions
// through them. #[expect] fails the build if the lint ever disappears.
#[expect(dead_code, reason = "spec-required status vocabulary")]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum VehicleStatus {
    Available,
    Reserved,
    Loaned,
    Lost,
    BeingServiced,
    Other,
}

#[expect(dead_code, reason = "spec-required status vocabulary")]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum ReservationStatus {
    Active,
    Pending,
    Confirmed,
    Completed,
    Cancelled,
    None,
}

#[expect(dead_code, reason = "spec-required status vocabulary")]
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum AccountStatus {
    Active,
    Closed,
    Canceled,
    Blacklisted,
    Blocked,
}

#[derive(Debug)]
struct Address {
    street: String,
    city: String,
    state: String,
    zip_code: String,
    country: String,
}

#[derive(Debug)]
struct Person {
    name: String,
    address: Address,
    email: String,
    phone: String,
}

#[derive(Debug)]
struct Account {
    id: String,
    password: String, // stores a hash in a real system; never printed
    status: AccountStatus,
    person: Person,
}

impl Account {
    fn password_matches(&self, candidate: &str) -> bool {
        // In production this is a constant-time hash comparison, not ==.
        self.password == candidate
    }
}

#[derive(Debug)]
struct Member {
    account: Account,
    total_vehicles_reserved: u32,
}

#[derive(Debug)]
struct Receptionist {
    account: Account,
    date_joined: NaiveDate,
}

/// A vehicle in the fleet. `price_per_day` is required by the spec and is
/// the basis for every reservation total.
#[derive(Debug)]
struct Vehicle {
    license_number: String,
    stock_number: String,
    capacity: u32,
    barcode: String,
    status: VehicleStatus,
    model: String,
    make: String,
    year: u32,
    mileage: u32,
    price_per_day: u32, // in cents
}

/// A reservation references the vehicle by barcode instead of owning a copy:
/// the vehicle's status and price live in one place, so a cancel or price
/// change can never diverge from what the reservation says.
#[derive(Debug, Clone)]
struct VehicleReservation {
    reservation_number: String,
    creation_date: NaiveDate,
    status: ReservationStatus,
    start_date: NaiveDate,
    end_date: NaiveDate, // inclusive
    pickup_location: String,
    return_location: String,
    customer_id: String,
    vehicle_barcode: String,
    total_price: u32, // in cents
    refunded: u32,
}

impl VehicleReservation {
    fn nights(&self) -> i64 {
        (self.end_date - self.start_date).num_days() + 1
    }
}

/// The payment contract from the requirements. Two implementations below.
trait PaymentProcessor: Send + Sync {
    fn charge(&self, amount_cents: u32, description: &str) -> Result<String, String>;
    fn refund(&self, txn_id: &str, amount_cents: u32) -> Result<(), String>;
}

#[derive(Clone, Copy)]
struct CreditCardPaymentProcessor;

impl PaymentProcessor for CreditCardPaymentProcessor {
    fn charge(&self, amount_cents: u32, description: &str) -> Result<String, String> {
        Ok(format!("CC-{description}-{amount_cents}"))
    }

    fn refund(&self, txn_id: &str, amount_cents: u32) -> Result<(), String> {
        println!("  [cc] refunded {amount_cents} cents for {txn_id}");
        Ok(())
    }
}

#[derive(Clone, Copy)]
struct PayPalPaymentProcessor;

impl PaymentProcessor for PayPalPaymentProcessor {
    fn charge(&self, amount_cents: u32, description: &str) -> Result<String, String> {
        Ok(format!("PP-{description}-{amount_cents}"))
    }

    fn refund(&self, txn_id: &str, amount_cents: u32) -> Result<(), String> {
        println!("  [paypal] refunded {amount_cents} cents for {txn_id}");
        Ok(())
    }
}

/// Core system. The spec's "Singleton + ConcurrentHashMap" becomes a
/// composition root behind a Mutex: one instance, shared via Arc, with every
/// mutating operation serialized so no two reservations can race.
#[derive(Debug)]
struct RentalSystem {
    name: String,
    locations: HashMap<String, Address>,
    vehicles: HashMap<String, Vehicle>,
    reservations: HashMap<String, VehicleReservation>,
    next_reservation: AtomicU64,
}

impl RentalSystem {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            locations: HashMap::new(),
            vehicles: HashMap::new(),
            reservations: HashMap::new(),
            next_reservation: AtomicU64::new(0),
        }
    }

    fn add_new_location(&mut self, name: String, address: Address) {
        self.locations.insert(name, address);
    }

    fn add_vehicle(&mut self, vehicle: Vehicle) {
        self.vehicles.insert(vehicle.barcode.clone(), vehicle);
    }

    fn remove_vehicle(&mut self, barcode: &str) -> Result<(), String> {
        let vehicle = self
            .vehicles
            .get(barcode)
            .ok_or_else(|| format!("vehicle {barcode} not found"))?;
        if vehicle.status == VehicleStatus::Loaned {
            return Err("cannot remove a vehicle that is currently loaned".to_string());
        }
        if self
            .reservations
            .values()
            .any(|r| r.vehicle_barcode == barcode && r.status == ReservationStatus::Confirmed)
        {
            return Err("cannot remove a vehicle with an active reservation".to_string());
        }
        self.vehicles.remove(barcode);
        Ok(())
    }

    /// True when no confirmed reservation overlaps [start, end] on the car.
    fn is_available(&self, barcode: &str, start: NaiveDate, end: NaiveDate) -> bool {
        self.reservations
            .values()
            .filter(|r| r.vehicle_barcode == barcode && r.status == ReservationStatus::Confirmed)
            .all(|r| end < r.start_date || start > r.end_date)
    }

    /// Search by make/model/year/price, restricted to the requested date
    /// range. A car must be physically rentable AND have no overlapping
    /// reservation to show up.
    fn search_vehicles(
        &self,
        make: Option<&str>,
        model: Option<&str>,
        year: Option<u32>,
        max_price_per_day: Option<u32>,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Vec<String> {
        let mut results: Vec<String> = self
            .vehicles
            .values()
            .filter(|v| v.status == VehicleStatus::Available)
            .filter(|v| make.is_none_or(|m| v.make == m))
            .filter(|v| model.is_none_or(|m| v.model == m))
            .filter(|v| year.is_none_or(|y| v.year == y))
            .filter(|v| max_price_per_day.is_none_or(|p| v.price_per_day <= p))
            .filter(|v| self.is_available(&v.barcode, start, end))
            .map(|v| v.barcode.clone())
            .collect();
        results.sort();
        results
    }

    /// Reserve a car for [start, end]. Total = price_per_day * nights.
    /// Payment happens before the reservation is committed; on payment
    /// failure nothing is recorded. All validation happens under the caller's
    /// lock, so two threads cannot reserve the same car twice.
    fn make_reservation(
        &mut self,
        customer_id: &str,
        vehicle_barcode: &str,
        start: NaiveDate,
        end: NaiveDate,
        pickup: &str,
        dropoff: &str,
        processor: &dyn PaymentProcessor,
    ) -> Result<String, String> {
        if end < start {
            return Err("end date cannot be before start date".to_string());
        }
        let vehicle = self
            .vehicles
            .get(vehicle_barcode)
            .ok_or_else(|| format!("vehicle {vehicle_barcode} not found"))?;
        if vehicle.status != VehicleStatus::Available {
            return Err(format!("vehicle is not available: {:?}", vehicle.status));
        }
        if !self.is_available(vehicle_barcode, start, end) {
            return Err(format!("vehicle {vehicle_barcode} is already reserved for those dates"));
        }

        let nights = (end - start).num_days() + 1;
        let total = vehicle.price_per_day as i64 * nights;
        let total: u32 = total.try_into().map_err(|_| "reservation total overflow")?;

        let reservation_number = format!("R-{:05}", self.next_reservation.fetch_add(1, Ordering::Relaxed) + 1);

        // Phase 1: record a Pending reservation (traceable even if payment
        // fails — it is rolled back below on failure).
        let mut reservation = VehicleReservation {
            reservation_number: reservation_number.clone(),
            creation_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            status: ReservationStatus::Pending,
            start_date: start,
            end_date: end,
            pickup_location: pickup.to_string(),
            return_location: dropoff.to_string(),
            customer_id: customer_id.to_string(),
            vehicle_barcode: vehicle_barcode.to_string(),
            total_price: total,
            refunded: 0,
        };
        self.reservations.insert(reservation_number.clone(), reservation.clone());

        // Phase 2: pay. On failure, roll back the pending reservation.
        match processor.charge(total, vehicle_barcode) {
            Ok(_txn) => {
                // Phase 3: commit.
                reservation.status = ReservationStatus::Confirmed;
                self.reservations.insert(reservation_number.clone(), reservation);
                Ok(reservation_number)
            }
            Err(err) => {
                self.reservations.remove(&reservation_number);
                Err(format!("payment failed: {err}"))
            }
        }
    }

    /// The rental begins: the car leaves the lot and becomes Loaned.
    fn start_rental(&mut self, reservation_number: &str, customer_id: &str) -> Result<(), String> {
        let mut reservation = self
            .reservations
            .get(reservation_number)
            .ok_or_else(|| format!("reservation {reservation_number} not found"))?
            .clone();
        if reservation.customer_id != customer_id {
            return Err("reservation belongs to another customer".to_string());
        }
        if reservation.status != ReservationStatus::Confirmed {
            return Err(format!("cannot start a {:?} reservation", reservation.status));
        }
        reservation.status = ReservationStatus::Active;
        self.reservations.insert(reservation_number.to_string(), reservation);
        if let Some(vehicle) = self.vehicles.get_mut(&self.reservations[reservation_number].vehicle_barcode) {
            vehicle.status = VehicleStatus::Loaned;
        }
        Ok(())
    }

    /// The rental ends: mark Completed and return the car to the pool.
    fn complete_rental(&mut self, reservation_number: &str, customer_id: &str) -> Result<(), String> {
        let mut reservation = self
            .reservations
            .get(reservation_number)
            .ok_or_else(|| format!("reservation {reservation_number} not found"))?
            .clone();
        if reservation.customer_id != customer_id {
            return Err("reservation belongs to another customer".to_string());
        }
        if reservation.status != ReservationStatus::Active {
            return Err(format!("cannot complete a {:?} reservation", reservation.status));
        }
        let barcode = reservation.vehicle_barcode.clone();
        reservation.status = ReservationStatus::Completed;
        self.reservations.insert(reservation_number.to_string(), reservation);
        if let Some(vehicle) = self.vehicles.get_mut(&barcode) {
            vehicle.status = VehicleStatus::Available;
        }
        Ok(())
    }

    /// Cancel: only the owning customer, only a confirmed reservation, only
    /// before the rental starts. Refunds the un-refunded remainder and frees
    /// the car for other customers.
    fn cancel_reservation(
        &mut self,
        reservation_number: &str,
        customer_id: &str,
        processor: &dyn PaymentProcessor,
    ) -> Result<(), String> {
        let mut reservation = self
            .reservations
            .get(reservation_number)
            .ok_or_else(|| format!("reservation {reservation_number} not found"))?
            .clone();
        if reservation.customer_id != customer_id {
            return Err("reservation belongs to another customer".to_string());
        }
        if reservation.status != ReservationStatus::Confirmed {
            return Err(format!("reservation is not confirmed: {:?}", reservation.status));
        }
        if NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() > reservation.start_date {
            return Err("cannot cancel a rental that has already started".to_string());
        }

        let refundable = reservation.total_price - reservation.refunded;
        reservation.refunded += refundable;
        reservation.status = ReservationStatus::Cancelled;
        self.reservations.insert(reservation_number.to_string(), reservation);
        if refundable > 0 {
            processor
                .refund(&format!("reservation-{reservation_number}"), refundable)
                .map_err(|e| format!("refund failed: {e}"))?;
        }
        Ok(())
    }

    /// Modify: shift the reservation to a new date range. The new range must
    /// not collide with other confirmed reservations for the same car. The
    /// price delta is charged or refunded.
    fn modify_reservation(
        &mut self,
        reservation_number: &str,
        customer_id: &str,
        new_start: NaiveDate,
        new_end: NaiveDate,
        processor: &dyn PaymentProcessor,
    ) -> Result<u32, String> {
        if new_end < new_start {
            return Err("end date cannot be before start date".to_string());
        }
        let reservation = self
            .reservations
            .get(reservation_number)
            .ok_or_else(|| format!("reservation {reservation_number} not found"))?
            .clone();
        if reservation.customer_id != customer_id {
            return Err("reservation belongs to another customer".to_string());
        }
        if reservation.status != ReservationStatus::Confirmed {
            return Err("cannot modify a non-confirmed reservation".to_string());
        }
        let vehicle = self
            .vehicles
            .get(&reservation.vehicle_barcode)
            .ok_or_else(|| format!("vehicle not found"))?;

        let conflict = self
            .reservations
            .values()
            .filter(|r| {
                r.vehicle_barcode == reservation.vehicle_barcode
                    && r.status == ReservationStatus::Confirmed
                    && r.reservation_number != reservation_number
            })
            .any(|r| new_start <= r.end_date && new_end >= r.start_date);
        if conflict {
            return Err("new dates overlap another reservation".to_string());
        }

        let nights = (new_end - new_start).num_days() + 1;
        let new_total: u32 = (vehicle.price_per_day as i64 * nights)
            .try_into()
            .map_err(|_| "reservation total overflow")?;

        let mut updated = reservation;
        updated.start_date = new_start;
        updated.end_date = new_end;
        let delta = new_total as i64 - updated.total_price as i64;
        if delta > 0 {
            processor
                .charge(delta as u32, &updated.vehicle_barcode)
                .map_err(|e| format!("payment failed: {e}"))?;
        } else if delta < 0 {
            processor
                .refund(&format!("reservation-{reservation_number}"), (-delta) as u32)
                .map_err(|e| format!("refund failed: {e}"))?;
            updated.refunded += (-delta) as u32;
        }
        updated.total_price = new_total;
        self.reservations.insert(reservation_number.to_string(), updated);
        Ok(delta.unsigned_abs() as u32)
    }
}

fn main() {
    let address = Address {
        street: "123 Main St".to_string(),
        city: "New York".to_string(),
        state: "NY".to_string(),
        zip_code: "10001".to_string(),
        country: "USA".to_string(),
    };

    // The spec's Singleton: build the system once and share it via Arc.
    let system = Arc::new(Mutex::new(RentalSystem::new("Global Car Rentals")));
    let processor = CreditCardPaymentProcessor;

    // The spec's account/member model, used at the branch for onboarding.
    let alice_account = Account {
        id: "CUST-001".to_string(),
        password: "hashed-•••".to_string(),
        status: AccountStatus::Active,
        person: Person {
            name: "Alice".to_string(),
            address: Address {
                street: "9 Fifth Ave".to_string(),
                city: "New York".to_string(),
                state: "NY".to_string(),
                zip_code: "10003".to_string(),
                country: "USA".to_string(),
            },
            email: "alice@example.com".to_string(),
            phone: "555-0100".to_string(),
        },
    };
    let alice_member = Member { account: alice_account, total_vehicles_reserved: 0 };
    let receptionist = Receptionist {
        account: Account {
            id: "REC-001".to_string(),
            password: "hashed-•••".to_string(),
            status: AccountStatus::Active,
            person: Person {
                name: "Rita".to_string(),
                address: Address {
                    street: "123 Main St".to_string(),
                    city: "New York".to_string(),
                    state: "NY".to_string(),
                    zip_code: "10001".to_string(),
                    country: "USA".to_string(),
                },
                email: "rita@example.com".to_string(),
                phone: "555-0101".to_string(),
            },
        },
        date_joined: NaiveDate::from_ymd_opt(2025, 3, 1).unwrap(),
    };

    {
        let mut sys = system.lock().unwrap();
        sys.add_new_location("NYC Branch".to_string(), address);

        let fleet = [
            ("VT-1001", "Toyota", "Camry", 2022, 50_00),
            ("VT-1002", "Toyota", "Corolla", 2021, 45_00),
            ("VT-1003", "Honda", "Civic", 2023, 55_00),
            ("VT-1004", "Ford", "Mustang", 2020, 90_00),
        ];
        for (barcode, make, model, year, price) in fleet {
            sys.add_vehicle(Vehicle {
                license_number: format!("{barcode}-LIC"),
                stock_number: format!("STK-{barcode}"),
                capacity: 5,
                barcode: barcode.to_string(),
                status: VehicleStatus::Available,
                model: model.to_string(),
                make: make.to_string(),
                year,
                mileage: 20_000,
                price_per_day: price,
            });
        }
        // One car out of service: it must never appear in search results.
        let civic = sys.vehicles.get_mut("VT-1003").unwrap();
        civic.status = VehicleStatus::BeingServiced;
        // A scrap car used only to exercise remove_vehicle.
        sys.add_vehicle(Vehicle {
            license_number: "VT-1099-LIC".to_string(),
            stock_number: "STK-1099".to_string(),
            capacity: 2,
            barcode: "VT-1099".to_string(),
            status: VehicleStatus::Available,
            model: "Beetle".to_string(),
            make: "VW".to_string(),
            year: 1995,
            mileage: 250_000,
            price_per_day: 20_00,
        });
        sys.remove_vehicle("VT-1099").unwrap();
    }

    let (branch_city, branch_street) = {
        let sys = system.lock().unwrap();
        let location = &sys.locations["NYC Branch"];
        (location.city.clone(), location.street.clone())
    };
    println!(
        "{} ({}) at {} ({}) lives in {} — {} {} · {} — booked {}",
        alice_member.account.person.name,
        alice_member.account.id,
        branch_city,
        branch_street,
        format!(
            "{}, {}, {} {}",
            alice_member.account.person.address.city,
            alice_member.account.person.address.state,
            alice_member.account.person.address.zip_code,
            alice_member.account.person.address.country
        ),
        alice_member.account.person.email,
        alice_member.account.person.phone,
        format!("{:?}", alice_member.account.status),
        alice_member.total_vehicles_reserved
    );
    println!(
        "  login check for {}: {}",
        alice_member.account.id,
        if alice_member.account.password_matches("hashed-•••") {
            "password verified"
        } else {
            "password rejected"
        }
    );
    println!(
        "  receptionist: {} since {}",
        receptionist.account.person.name, receptionist.date_joined
    );

    let start = NaiveDate::from_ymd_opt(2026, 5, 10).unwrap();
    let end = NaiveDate::from_ymd_opt(2026, 5, 12).unwrap();

    // Fleet summary (reads the fields the spec requires on Vehicle).
    {
        let sys = system.lock().unwrap();
        println!("  fleet at {}:", sys.name);
        for vehicle in sys.vehicles.values() {
            println!(
                "    {} {} {} ({} · {} · {} km, {}-seater) — ₹{}/day, {:?}",
                vehicle.make,
                vehicle.model,
                vehicle.year,
                vehicle.license_number,
                vehicle.stock_number,
                vehicle.mileage,
                vehicle.capacity,
                vehicle.price_per_day / 100,
                vehicle.status
            );
        }
    }

    // Search: all available Toyotas under ₹700/day for those dates.
    let available = system
        .lock()
        .unwrap()
        .search_vehicles(Some("Toyota"), None, None, Some(70_00), start, end);
    println!("Available Toyotas under ₹700/day: {available:?}");

    // Reserve the Camry for Alice (credit card processor).
    let alice = "CUST-001";
    let res = system.lock().unwrap().make_reservation(
        alice,
        "VT-1001",
        start,
        end,
        "NYC Branch",
        "NYC Branch",
        &processor,
    );
    println!("Alice's reservation: {res:?}");
    if let Ok(number) = &res {
        let sys = system.lock().unwrap();
        let booking = &sys.reservations[number];
        println!(
            "  created {} for {} nights — pickup {} → return {}, total ₹{}\n",
            booking.creation_date,
            booking.nights(),
            booking.pickup_location,
            booking.return_location,
            booking.total_price / 100
        );
    }

    // Bug 1 test: Bob tries the same car, overlapping dates → must fail.
    let bob = "CUST-002";
    let res2 = system.lock().unwrap().make_reservation(
        bob,
        "VT-1001",
        start,
        end,
        "NYC Branch",
        "NYC Branch",
        &processor,
    );
    println!("Bob's overlapping reservation: {res2:?}");

    // Search again: the Camry must be gone from the results.
    let available_after = system
        .lock()
        .unwrap()
        .search_vehicles(Some("Toyota"), None, None, Some(70_00), start, end);
    println!("Available Toyotas after Alice's booking: {available_after:?}");

    // Bob books the Corolla for a later week — via the PayPal processor.
    let later_start = NaiveDate::from_ymd_opt(2026, 5, 20).unwrap();
    let later_end = NaiveDate::from_ymd_opt(2026, 5, 21).unwrap();
    let paypal = PayPalPaymentProcessor;
    let res3 = system.lock().unwrap().make_reservation(
        bob,
        "VT-1002",
        later_start,
        later_end,
        "NYC Branch",
        "NYC Branch",
        &paypal,
    );
    println!("Bob's reservation (PayPal): {res3:?}");

    // Full lifecycle on Bob's reservation: start rental, complete rental.
    let res3 = res3.unwrap();
    system.lock().unwrap().start_rental(&res3, bob).unwrap();
    println!("  {res3} → Loaned");
    system.lock().unwrap().complete_rental(&res3, bob).unwrap();
    println!("  {res3} → Completed, car back to Available");

    // Alice modifies hers: extend by one day → extra charge.
    let extended_end = NaiveDate::from_ymd_opt(2026, 5, 13).unwrap();
    let delta = system.lock().unwrap().modify_reservation(
        res.as_ref().unwrap(),
        alice,
        start,
        extended_end,
        &processor,
    );
    println!("Alice extends by a day → extra charged: {delta:?}");

    // Alice cancels hers → refund, car free again.
    system
        .lock()
        .unwrap()
        .cancel_reservation(res.as_ref().unwrap(), alice, &processor)
        .unwrap();
    let available_final = system
        .lock()
        .unwrap()
        .search_vehicles(Some("Toyota"), None, None, Some(70_00), start, end);
    println!("Available Toyotas after Alice cancels: {available_final:?}");

    println!("\nCar Rental System initialized");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn setup() -> (Arc<Mutex<RentalSystem>>, CreditCardPaymentProcessor) {
        let system = Arc::new(Mutex::new(RentalSystem::new("Test")));
        let processor = CreditCardPaymentProcessor;
        {
            let mut sys = system.lock().unwrap();
            sys.add_vehicle(Vehicle {
                license_number: "T-1".to_string(),
                stock_number: "S-1".to_string(),
                capacity: 5,
                barcode: "B-1".to_string(),
                status: VehicleStatus::Available,
                model: "Camry".to_string(),
                make: "Toyota".to_string(),
                year: 2022,
                mileage: 10,
                price_per_day: 50_00,
            });
        }
        (system, processor)
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn test_booking_flow() {
        let (system, processor) = setup();
        let res = system
            .lock()
            .unwrap()
            .make_reservation("C1", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor)
            .unwrap();
        assert!(res.starts_with("R-"));
    }

    #[test]
    fn test_overlap_rejected() {
        let (system, processor) = setup();
        let mut sys = system.lock().unwrap();
        sys.make_reservation("C1", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor)
            .unwrap();
        // Same dates and an overlapping window both must fail.
        assert!(sys.make_reservation("C2", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor).is_err());
        assert!(sys.make_reservation("C2", "B-1", d(2026, 5, 11), d(2026, 5, 13), "NYC", "NYC", &processor).is_err());
        // Adjacent window (ends before start) is fine.
        assert!(sys.make_reservation("C2", "B-1", d(2026, 5, 13), d(2026, 5, 14), "NYC", "NYC", &processor).is_ok());
    }

    #[test]
    fn test_concurrent_reservation_race() {
        let (system, processor) = setup();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let system = system.clone();
                thread::spawn(move || {
                    let mut sys = system.lock().unwrap();
                    sys.make_reservation(
                        &format!("C{i}"),
                        "B-1",
                        d(2026, 5, 10),
                        d(2026, 5, 12),
                        "NYC",
                        "NYC",
                        &processor,
                    )
                    .is_ok()
                })
            })
            .collect();
        let winners = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(winners, 1, "exactly one concurrent reservation must win");
    }

    #[test]
    fn test_cancel_refunds_and_frees() {
        let (system, processor) = setup();
        let mut sys = system.lock().unwrap();
        let res = sys
            .make_reservation("C1", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor)
            .unwrap();
        sys.cancel_reservation(&res, "C1", &processor).unwrap();
        // The car is bookable again for the same dates.
        assert!(sys.make_reservation("C2", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor).is_ok());
    }

    #[test]
    fn test_cancel_wrong_customer() {
        let (system, processor) = setup();
        let mut sys = system.lock().unwrap();
        let res = sys
            .make_reservation("C1", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor)
            .unwrap();
        assert!(sys.cancel_reservation(&res, "C2", &processor).is_err());
    }

    #[test]
    fn test_search_respects_availability() {
        let (system, processor) = setup();
        let mut sys = system.lock().unwrap();
        sys.make_reservation("C1", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor)
            .unwrap();
        // Booked dates: the car must not appear.
        let hits = sys.search_vehicles(None, None, None, None, d(2026, 5, 10), d(2026, 5, 12));
        assert!(hits.is_empty());
        // Non-overlapping dates: it appears again.
        let hits = sys.search_vehicles(None, None, None, None, d(2026, 5, 20), d(2026, 5, 21));
        assert_eq!(hits, vec!["B-1".to_string()]);
    }

    #[test]
    fn test_modify_rejects_overlap() {
        let (system, processor) = setup();
        let mut sys = system.lock().unwrap();
        let res = sys
            .make_reservation("C1", "B-1", d(2026, 5, 10), d(2026, 5, 12), "NYC", "NYC", &processor)
            .unwrap();
        // Shift into a date range that collides with... itself is allowed, but
        // a range that a second reservation holds must be rejected.
        sys.make_reservation("C2", "B-1", d(2026, 5, 20), d(2026, 5, 21), "NYC", "NYC", &processor)
            .unwrap();
        let err = sys
            .modify_reservation(&res, "C1", d(2026, 5, 20), d(2026, 5, 21), &processor)
            .is_err();
        assert!(err);
    }
}
