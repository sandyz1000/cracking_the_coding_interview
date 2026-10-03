use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::domain::accounts::{Passenger, User, UserRole};
use crate::domain::bookings::BookingManager;
use crate::domain::flights::{Aircraft, CrewMember, Flight, FlightRegistry, FlightSpec};
use crate::domain::payments::{PaymentGateway, PaymentProcessor};
use crate::domain::search::FlightSearch;
use crate::error::{AmsError, AmsResult};
use crate::locks::{rd, wr};

/// Composition root. Every shared component is built here exactly once and
/// handed out via `Arc` — Rust's answer to the Singleton pattern, minus
/// process-global state.
pub struct AirlineManagementSystem {
    pub flights: Arc<FlightRegistry>,
    pub flight_search: Arc<FlightSearch>,
    pub bookings: Arc<BookingManager>,
    pub payments: Arc<PaymentProcessor>,
    users: RwLock<HashMap<u64, User>>,
    passengers: RwLock<HashMap<u64, Passenger>>,
    next_id: AtomicU64,
    next_flight_number: AtomicU64,
}

impl AirlineManagementSystem {
    pub fn with_gateway(gateway: Box<dyn PaymentGateway>) -> Self {
        let flights: Arc<FlightRegistry> = Arc::new(RwLock::new(HashMap::new()));
        let payments = Arc::new(PaymentProcessor::new(gateway));
        let bookings = Arc::new(BookingManager::new(flights.clone(), payments.clone()));
        let flight_search = Arc::new(FlightSearch::new(flights.clone()));
        Self {
            flights,
            flight_search,
            bookings,
            payments,
            users: RwLock::new(HashMap::new()),
            passengers: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(0),
            next_flight_number: AtomicU64::new(0),
        }
    }

    fn ensure_admin(&self, actor: &User) -> AmsResult<()> {
        if actor.is_admin() {
            Ok(())
        } else {
            Err(AmsError::PermissionDenied)
        }
    }

    pub fn register_passenger(&self, name: &str, email: &str, phone: &str) -> Passenger {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let passenger = Passenger {
            id,
            name: name.to_string(),
            email: email.to_string(),
            phone: phone.to_string(),
        };
        wr(&self.passengers).insert(id, passenger.clone());
        wr(&self.users).insert(id, User::new(id, name, UserRole::Passenger));
        passenger
    }

    pub fn user(&self, id: u64) -> Option<User> {
        let guard = rd(&self.users);
        guard.get(&id).cloned()
    }

    pub fn passenger(&self, id: u64) -> Option<Passenger> {
        let guard = rd(&self.passengers);
        guard.get(&id).cloned()
    }

    pub fn schedule_flight(
        &self,
        actor: &User,
        aircraft: Arc<Aircraft>,
        spec: FlightSpec,
        base_fare: f64,
    ) -> AmsResult<Arc<Flight>> {
        self.ensure_admin(actor)?;
        let flight_number = format!(
            "AI{:04}",
            self.next_flight_number.fetch_add(1, Ordering::Relaxed) + 1
        );
        let flight = Arc::new(Flight::new(flight_number, spec, aircraft, base_fare));
        {
            let mut flights = wr(&self.flights);
            flights.insert(flight.flight_number.clone(), flight.clone());
        }
        self.flight_search.rebuild_index();
        Ok(flight)
    }

    pub fn change_aircraft(
        &self,
        actor: &User,
        flight_number: &str,
        aircraft: Arc<Aircraft>,
    ) -> AmsResult<()> {
        self.ensure_admin(actor)?;
        let flight = rd(&self.flights)
            .get(flight_number)
            .cloned()
            .ok_or_else(|| AmsError::FlightNotFound(flight_number.to_string()))?;
        flight.swap_aircraft(aircraft)
    }

    pub fn assign_crew(
        &self,
        actor: &User,
        flight_number: &str,
        crew: Vec<CrewMember>,
    ) -> AmsResult<()> {
        self.ensure_admin(actor)?;
        let flight = rd(&self.flights)
            .get(flight_number)
            .cloned()
            .ok_or_else(|| AmsError::FlightNotFound(flight_number.to_string()))?;
        flight.assign_crew(crew);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::flights::CrewRole;
    use crate::domain::payments::PaymentMethod;
    use crate::test_util;
    use crate::time::{Date, Time};

    #[test]
    fn test_role_permissions() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let owner = test_util::passenger(&ams, "Owner");
        let other = test_util::passenger(&ams, "Other");
        let staff = User::new(9, "Staff", UserRole::Staff);

        let (booking, _) = ams
            .bookings
            .book(&owner, &flight.flight_number, "1A", PaymentMethod::Card)
            .unwrap();
        let err = ams
            .bookings
            .cancel(booking.booking_number, &other)
            .unwrap_err();
        assert!(matches!(err, AmsError::PermissionDenied));

        assert!(matches!(
            ams.schedule_flight(
                &owner,
                Arc::new(Aircraft {
                    tail_number: "X".into(),
                    model: "X".into(),
                    total_seats: 36,
                }),
                FlightSpec {
                    source: "BOM".into(),
                    destination: "DEL".into(),
                    date: Date::new(2026, 1, 2),
                    departure: Time::new(9, 0),
                    arrival: Time::new(11, 0),
                },
                100.0,
            ),
            Err(AmsError::PermissionDenied)
        ));

        // Staff can cancel on the passenger's behalf; the seat returns to the pool.
        ams.bookings.cancel(booking.booking_number, &staff).unwrap();
        ams.bookings
            .book(&owner, &flight.flight_number, "1A", PaymentMethod::Card)
            .unwrap();
    }

    #[test]
    fn test_crew_assignment() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let admin = User::new(1, "Admin", UserRole::Admin);
        let crew = vec![
            CrewMember {
                id: 1,
                name: "Capt. Sharma".into(),
                role: CrewRole::Captain,
            },
            CrewMember {
                id: 2,
                name: "Priya (CC)".into(),
                role: CrewRole::CabinCrew,
            },
        ];

        ams.assign_crew(&admin, &flight.flight_number, crew)
            .unwrap();
        assert_eq!(flight.crew_manifest().len(), 2);
    }
}
