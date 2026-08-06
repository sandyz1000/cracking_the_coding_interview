//! Airline management system. Design decisions: see `DESIGN.md`.

pub mod adapters;
pub mod domain;
pub mod error;
pub(crate) mod locks;
pub mod time;

pub use domain::accounts::{Passenger, User, UserRole};
pub use domain::bookings::{
    Baggage, Booking, BookingManager, BookingStatus, CancelResult, ChangeResult,
};
pub use domain::flights::{
    Aircraft, CrewMember, CrewRole, Flight, FlightRegistry, FlightSnapshot, FlightSpec, SeatRecord,
    SeatStatus, SeatType,
};
pub use domain::payments::{
    Payment, PaymentGateway, PaymentMethod, PaymentProcessor, PaymentStatus,
};
pub use domain::search::FlightSearch;
pub use domain::system::AirlineManagementSystem;
pub use error::{AmsError, AmsResult};

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;
    use crate::adapters::gateway::MockGateway;
    use crate::time::{Date, Time};
    use std::sync::Arc;

    pub(crate) fn system() -> Arc<AirlineManagementSystem> {
        Arc::new(AirlineManagementSystem::with_gateway(Box::new(
            MockGateway::default(),
        )))
    }

    pub(crate) fn flight_with(
        ams: &AirlineManagementSystem,
        tail: &str,
        base_fare: f64,
        departure: Time,
    ) -> Arc<Flight> {
        let admin = User::new(1, "Admin", UserRole::Admin);
        let spec = FlightSpec {
            source: "DEL".into(),
            destination: "BOM".into(),
            date: Date::new(2026, 1, 1),
            departure,
            arrival: Time::new(10, 0),
        };
        let aircraft = Arc::new(Aircraft {
            tail_number: tail.into(),
            model: "TestJet".into(),
            total_seats: 36,
        });
        ams.schedule_flight(&admin, aircraft, spec, base_fare)
            .expect("admin schedules flight")
    }

    pub(crate) fn flight(ams: &AirlineManagementSystem) -> Arc<Flight> {
        flight_with(ams, "TEST-1", 100.0, Time::new(8, 0))
    }

    pub(crate) fn passenger(ams: &AirlineManagementSystem, name: &str) -> User {
        let person = ams.register_passenger(name, "x@y.z", "123");
        ams.user(person.id).expect("passenger registered")
    }
}
