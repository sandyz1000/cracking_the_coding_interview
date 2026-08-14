//! Car rental system.
//!
//! Spec: `readme.md`. Design decisions taken on top of it: `DESIGN.md`.

pub mod adapters;
pub mod domain;
pub mod error;
pub(crate) mod locks;
pub mod time;

pub use domain::accounts::{Address, Customer, User, UserRole};
pub use domain::payments::{
    Payment, PaymentGateway, PaymentMethod, PaymentProcessor, PaymentStatus,
};
pub use domain::reservations::{
    Reservation, ReservationBook, ReservationManager, ReservationSpec, ReservationStatus,
};
pub use domain::search::CarSearch;
pub use domain::system::CarRentalSystem;
pub use domain::vehicles::{Vehicle, VehicleRegistry, VehicleSnapshot, VehicleSpec, VehicleStatus};
pub use error::{CarError, CarResult};

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;
    use crate::adapters::gateway::MockGateway;
    use std::sync::Arc;

    pub(crate) fn system() -> Arc<CarRentalSystem> {
        Arc::new(CarRentalSystem::with_gateway(Box::new(
            MockGateway::default(),
        )))
    }

    pub(crate) fn customer(system: &CarRentalSystem, name: &str) -> User {
        let customer = system.register_customer(name, "x@y.z", "123", "LIC-1");
        system
            .user(customer.id)
            .expect("customer registered as user")
    }

    pub(crate) fn vehicle(barcode: &str) -> Vehicle {
        Vehicle::new(VehicleSpec {
            barcode: barcode.into(),
            license_number: format!("{barcode}-LIC"),
            stock_number: format!("STK-{barcode}"),
            capacity: 5,
            make: "Toyota".into(),
            model: "Camry".into(),
            year: 2022,
            mileage: 20_000,
            price_per_day: 5_000,
        })
    }
}
