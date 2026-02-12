use std::collections::HashMap;
use chrono::{NaiveDate, Utc};

// Enums for various statuses and types
#[derive(Debug, PartialEq, Eq, Hash)]
enum VehicleStatus {
    Available,
    Reserved,
    Loaned,
    Lost,
    BeingServiced,
    Other,
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ReservationStatus {
    Active,
    Pending,
    Confirmed,
    Completed,
    Cancelled,
    None,
}

#[derive(Debug, PartialEq, Eq, Hash)]
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
    password: String,
    status: AccountStatus,
    person: Person,
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
}

#[derive(Debug)]
struct VehicleReservation {
    reservation_number: String,
    creation_date: NaiveDate,
    status: ReservationStatus,
    due_date: NaiveDate,
    return_date: Option<NaiveDate>,
    pickup_location: String,
    return_location: String,
    customer_id: String,
    vehicle: Vehicle,
}

#[derive(Debug)]
struct CarRentalSystem {
    name: String,
    locations: HashMap<String, Address>,
    vehicles: HashMap<String, Vehicle>,
    reservations: HashMap<String, VehicleReservation>,
}

impl CarRentalSystem {
    fn add_new_location(&mut self, name: String, address: Address) {
        self.locations.insert(name, address);
    }
    
    fn add_vehicle(&mut self, vehicle: Vehicle) {
        self.vehicles.insert(vehicle.barcode.clone(), vehicle);
    }
    
    fn make_reservation(&mut self, reservation: VehicleReservation) {
        self.reservations.insert(reservation.reservation_number.clone(), reservation);
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
    
    let mut system = CarRentalSystem {
        name: "Global Car Rentals".to_string(),
        locations: HashMap::new(),
        vehicles: HashMap::new(),
        reservations: HashMap::new(),
    };
    
    system.add_new_location("NYC Branch".to_string(), address);
    println!("Car Rental System initialized: {:?}", system);
}

