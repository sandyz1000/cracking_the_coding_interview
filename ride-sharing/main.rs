//! Ride-sharing service. Full requirements: see `readme.md`. Same
//! architectural DNA as restaurant_management/library_management/
//! music_streaming (`OnceLock` singleton composition root, `RwLock`-guarded
//! maps, thiserror, strum `Display`), scaled to this domain's real
//! contention point: two drivers racing to accept the same ride request
//! must never both win.

/*
# Designing a Ride-Sharing Service Like Uber

## Requirements

1. The ride sharing service should allow passengers to request rides and drivers to accept and fulfill those
ride requests.
2. Passengers should be able to specify their pickup location, destination, and desired ride type (e.g.,
regular, premium).
3. Drivers should be able to see available ride requests and choose to accept or decline them.
4. The system should match ride requests with available drivers based on proximity and other factors.
5. The system should calculate the fare for each ride based on distance, time, and ride type.
6. The system should handle payments and process transactions between passengers and drivers.
7. The system should provide real-time tracking of ongoing rides and notify passengers and drivers about ride
status updates.
8. The system should handle concurrent requests and ensure data consistency.

## Classes, Interfaces and Enumerations

1. The **Passenger** class represents a passenger in the ride sharing service, with properties such as ID,
name, contact information, and location.
2. The **Driver** class represents a driver in the ride sharing service, with properties such as ID, name,
contact information, license plate, location, and status (available or busy).
3. The **Ride** class represents a ride requested by a passenger and accepted by a driver, with properties
such as ID, passenger, driver, source location, destination location, status, and fare.
4. The **Location** class represents a geographical location with latitude and longitude coordinates.
5. The **Payment** class represents a payment made for a ride, with properties such as ID, ride, amount, and
payment status.
6. The **RideService** class is the main class that manages the ride sharing service. It follows the Singleton
pattern to ensure only one instance of the service exists.
7. The RideService class provides methods for adding passengers and drivers, requesting rides, accepting
rides, starting rides, completing rides, and canceling rides.
8. Multi-threading is implemented using concurrent data structures (ConcurrentHashMap and ConcurrentLinkedQueue)
to handle concurrent access to shared data, such as ride requests and driver availability.
9. The notifyDrivers, notifyPassenger, and notifyDriver methods are placeholders for notifying relevant parties
about ride status updates.
10. The calculateFare and processPayment methods are placeholders for calculating ride fares and processing
payments, respectively.
11. The **RideSharingDemo** class demonstrates the usage of the ride sharing service by creating passengers and
drivers, requesting rides, accepting rides, starting rides, completing rides, and canceling rides.
*/

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;

fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    // A poisoned lock still holds valid data; the panic happened in an
    // earlier thread, not because the data is corrupt.
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum DriverStatus {
    Available,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum RideStatus {
    Requested,
    Accepted,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum RideType {
    Regular,
    Premium,
}

impl RideType {
    /// Requirement #5: ride type is one of the fare inputs.
    fn fare_multiplier(self) -> f64 {
        match self {
            RideType::Regular => 1.0,
            RideType::Premium => 1.6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum PaymentStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RideError {
    #[error("passenger {id} not found")]
    PassengerNotFound { id: uuid::Uuid },
    #[error("driver {id} not found")]
    DriverNotFound { id: uuid::Uuid },
    #[error("ride {id} not found")]
    RideNotFound { id: uuid::Uuid },
    #[error("no drivers available")]
    NoDriversAvailable,
    #[error("driver {id} is not available")]
    DriverNotAvailable { id: uuid::Uuid },
    #[error("ride {id} is not awaiting acceptance (status: {status})")]
    RideNotRequested { id: uuid::Uuid, status: RideStatus },
    #[error("ride {id} is not accepted yet (status: {status})")]
    RideNotAccepted { id: uuid::Uuid, status: RideStatus },
    #[error("ride {id} is not in progress (status: {status})")]
    RideNotInProgress { id: uuid::Uuid, status: RideStatus },
    #[error("ride {id} already completed, cannot cancel")]
    RideAlreadyCompleted { id: uuid::Uuid },
}

pub type RideResult<T> = std::result::Result<T, RideError>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

impl Location {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude,
            longitude,
        }
    }

    /// Great-circle distance in km (haversine) — correct on the globe,
    /// unlike a flat euclidean distance on raw lat/long degrees.
    fn distance_km(&self, other: &Location) -> f64 {
        const EARTH_RADIUS_KM: f64 = 6371.0;
        let (lat1, lat2) = (self.latitude.to_radians(), other.latitude.to_radians());
        let d_lat = (other.latitude - self.latitude).to_radians();
        let d_lon = (other.longitude - self.longitude).to_radians();
        let a = (d_lat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().asin();
        EARTH_RADIUS_KM * c
    }
}

#[derive(Debug, Clone)]
pub struct Passenger {
    pub id: uuid::Uuid,
    pub name: String,
    pub phone: String,
    pub location: Location,
}

impl Passenger {
    fn new(name: impl Into<String>, phone: impl Into<String>, location: Location) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            phone: phone.into(),
            location,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Driver {
    pub id: uuid::Uuid,
    pub name: String,
    pub phone: String,
    pub license_plate: String,
    pub location: Location,
    status: DriverStatus,
}

impl Driver {
    fn new(
        name: impl Into<String>,
        phone: impl Into<String>,
        license_plate: impl Into<String>,
        location: Location,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            phone: phone.into(),
            license_plate: license_plate.into(),
            location,
            status: DriverStatus::Available,
        }
    }

    pub fn status(&self) -> DriverStatus {
        self.status
    }
}

#[derive(Debug, Clone)]
pub struct Ride {
    pub id: uuid::Uuid,
    pub passenger_id: uuid::Uuid,
    pub driver_id: Option<uuid::Uuid>,
    pub source: Location,
    pub destination: Location,
    pub ride_type: RideType,
    pub status: RideStatus,
    pub fare: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub id: uuid::Uuid,
    pub ride_id: uuid::Uuid,
    pub amount: f64,
    pub status: PaymentStatus,
}

/// Singleton control point for the service (requirement #6).
pub struct RideService {
    passengers: RwLock<HashMap<uuid::Uuid, Passenger>>,
    drivers: RwLock<HashMap<uuid::Uuid, Driver>>,
    // Lock order everywhere below is always rides -> drivers, so
    // accept_ride and complete_ride (which touch both) can never deadlock
    // against each other.
    rides: RwLock<HashMap<uuid::Uuid, Ride>>,
    payments: RwLock<HashMap<uuid::Uuid, Payment>>,
}

impl RideService {
    const BASE_FARE: f64 = 2.5;
    const RATE_PER_KM: f64 = 1.2;
    const RATE_PER_MIN: f64 = 0.25;

    pub fn instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<RideService>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(RideService::new()))
    }

    pub fn new() -> Self {
        Self {
            passengers: RwLock::new(HashMap::new()),
            drivers: RwLock::new(HashMap::new()),
            rides: RwLock::new(HashMap::new()),
            payments: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_passenger(&self, name: &str, phone: &str, location: Location) -> Passenger {
        let passenger = Passenger::new(name, phone, location);
        write_guard(&self.passengers).insert(passenger.id, passenger.clone());
        passenger
    }

    pub fn add_driver(
        &self,
        name: &str,
        phone: &str,
        license_plate: &str,
        location: Location,
    ) -> Driver {
        let driver = Driver::new(name, phone, license_plate, location);
        write_guard(&self.drivers).insert(driver.id, driver.clone());
        driver
    }

    pub fn driver_status(&self, id: uuid::Uuid) -> RideResult<DriverStatus> {
        read_guard(&self.drivers)
            .get(&id)
            .map(|d| d.status)
            .ok_or(RideError::DriverNotFound { id })
    }

    /// Requirement #4: nearest available driver to a pickup location.
    fn nearest_available_driver(&self, pickup: &Location) -> Option<uuid::Uuid> {
        read_guard(&self.drivers)
            .values()
            .filter(|driver| driver.status == DriverStatus::Available)
            .min_by(|a, b| {
                a.location
                    .distance_km(pickup)
                    .total_cmp(&b.location.distance_km(pickup))
            })
            .map(|driver| driver.id)
    }

    /// Requirement #1/#2: a passenger requests a ride. Broadcasts to every
    /// available driver (placeholder notification, requirement #9) — any of
    /// them may then call `accept_ride`.
    pub fn request_ride(
        &self,
        passenger_id: uuid::Uuid,
        source: Location,
        destination: Location,
        ride_type: RideType,
    ) -> RideResult<Ride> {
        if !read_guard(&self.passengers).contains_key(&passenger_id) {
            return Err(RideError::PassengerNotFound { id: passenger_id });
        }
        if self.nearest_available_driver(&source).is_none() {
            return Err(RideError::NoDriversAvailable);
        }
        let ride = Ride {
            id: uuid::Uuid::new_v4(),
            passenger_id,
            driver_id: None,
            source,
            destination,
            ride_type,
            status: RideStatus::Requested,
            fare: None,
        };
        write_guard(&self.rides).insert(ride.id, ride.clone());
        self.notify_drivers(&ride);
        Ok(ride)
    }

    /// Requirement #3/#8: any driver can see and accept an open request, but
    /// exactly one wins. The ride and driver rows are locked together for
    /// the whole check-then-commit, so two drivers racing on the same ride
    /// can never both be assigned it.
    pub fn accept_ride(&self, ride_id: uuid::Uuid, driver_id: uuid::Uuid) -> RideResult<()> {
        let mut rides = write_guard(&self.rides);
        let mut drivers = write_guard(&self.drivers);

        let ride = rides
            .get_mut(&ride_id)
            .ok_or(RideError::RideNotFound { id: ride_id })?;
        if ride.status != RideStatus::Requested {
            return Err(RideError::RideNotRequested {
                id: ride_id,
                status: ride.status,
            });
        }
        let driver = drivers
            .get_mut(&driver_id)
            .ok_or(RideError::DriverNotFound { id: driver_id })?;
        if driver.status != DriverStatus::Available {
            return Err(RideError::DriverNotAvailable { id: driver_id });
        }

        driver.status = DriverStatus::Busy;
        ride.driver_id = Some(driver_id);
        ride.status = RideStatus::Accepted;
        self.notify_passenger(ride, &format!("driver {} accepted your ride", driver.name));
        Ok(())
    }

    pub fn start_ride(&self, ride_id: uuid::Uuid) -> RideResult<()> {
        let mut rides = write_guard(&self.rides);
        let ride = rides
            .get_mut(&ride_id)
            .ok_or(RideError::RideNotFound { id: ride_id })?;
        if ride.status != RideStatus::Accepted {
            return Err(RideError::RideNotAccepted {
                id: ride_id,
                status: ride.status,
            });
        }
        ride.status = RideStatus::InProgress;
        self.notify_passenger(ride, "your ride has started");
        Ok(())
    }

    /// Requirement #5/#6: fare from distance, time, and ride type, then a
    /// (placeholder) payment. Frees the driver back to `Available`.
    pub fn complete_ride(&self, ride_id: uuid::Uuid, duration_mins: f64) -> RideResult<Payment> {
        let mut rides = write_guard(&self.rides);
        let mut drivers = write_guard(&self.drivers);

        let ride = rides
            .get_mut(&ride_id)
            .ok_or(RideError::RideNotFound { id: ride_id })?;
        if ride.status != RideStatus::InProgress {
            return Err(RideError::RideNotInProgress {
                id: ride_id,
                status: ride.status,
            });
        }
        let distance_km = ride.source.distance_km(&ride.destination);
        let fare = Self::calculate_fare(ride.ride_type, distance_km, duration_mins);
        ride.status = RideStatus::Completed;
        ride.fare = Some(fare);

        if let Some(driver_id) = ride.driver_id
            && let Some(driver) = drivers.get_mut(&driver_id)
        {
            driver.status = DriverStatus::Available;
        }

        let payment = self.process_payment(ride_id, fare);
        self.notify_passenger(ride, &format!("ride completed, fare ${fare:.2}"));
        if let Some(driver_id) = ride.driver_id {
            self.notify_driver(
                driver_id,
                &format!("ride {ride_id} completed, fare ${fare:.2}"),
            );
        }
        Ok(payment)
    }

    pub fn cancel_ride(&self, ride_id: uuid::Uuid) -> RideResult<()> {
        let mut rides = write_guard(&self.rides);
        let mut drivers = write_guard(&self.drivers);

        let ride = rides
            .get_mut(&ride_id)
            .ok_or(RideError::RideNotFound { id: ride_id })?;
        if ride.status == RideStatus::Completed {
            return Err(RideError::RideAlreadyCompleted { id: ride_id });
        }
        if let Some(driver_id) = ride.driver_id
            && let Some(driver) = drivers.get_mut(&driver_id)
        {
            driver.status = DriverStatus::Available;
        }
        ride.status = RideStatus::Cancelled;
        self.notify_passenger(ride, "your ride was cancelled");
        Ok(())
    }

    /// Requirement #5: distance x rate + time x rate, scaled by ride type.
    fn calculate_fare(ride_type: RideType, distance_km: f64, duration_mins: f64) -> f64 {
        (Self::BASE_FARE + Self::RATE_PER_KM * distance_km + Self::RATE_PER_MIN * duration_mins)
            * ride_type.fare_multiplier()
    }

    /// Requirement #10 placeholder: settles the fare against the passenger.
    fn process_payment(&self, ride_id: uuid::Uuid, amount: f64) -> Payment {
        let payment = Payment {
            id: uuid::Uuid::new_v4(),
            ride_id,
            amount,
            status: PaymentStatus::Completed,
        };
        write_guard(&self.payments).insert(payment.id, payment.clone());
        payment
    }

    // Requirement #9 placeholders: real-time status notifications.
    fn notify_drivers(&self, ride: &Ride) {
        println!(
            "  [notify drivers] new {} ride request {} pickup ({:.4}, {:.4})",
            ride.ride_type, ride.id, ride.source.latitude, ride.source.longitude
        );
    }

    fn notify_passenger(&self, ride: &Ride, message: &str) {
        println!(
            "  [notify passenger {}] ride {}: {message}",
            ride.passenger_id, ride.id
        );
    }

    fn notify_driver(&self, driver_id: uuid::Uuid, message: &str) {
        println!("  [notify driver {driver_id}] {message}");
    }
}

impl Default for RideService {
    fn default() -> Self {
        Self::new()
    }
}

fn run_demo() {
    let service = RideService::instance();

    let alice = service.add_passenger("Alice", "555-0100", Location::new(37.7749, -122.4194));
    let bob = service.add_driver(
        "Bob",
        "555-0200",
        "7XYZ123",
        Location::new(37.7750, -122.4180),
    );
    let carol = service.add_driver(
        "Carol",
        "555-0201",
        "8ABC456",
        Location::new(37.8000, -122.4500),
    );

    println!("== Request ride ==");
    let ride = service
        .request_ride(
            alice.id,
            alice.location,
            Location::new(37.8044, -122.2712),
            RideType::Premium,
        )
        .expect("ride requested");
    assert_eq!(ride.status, RideStatus::Requested);

    println!("\n== Accept ride (Bob is closer, wins) ==");
    service.accept_ride(ride.id, bob.id).expect("bob accepts");
    assert_eq!(service.driver_status(bob.id).unwrap(), DriverStatus::Busy);
    assert_eq!(
        service.driver_status(carol.id).unwrap(),
        DriverStatus::Available
    );
    match service.accept_ride(ride.id, carol.id) {
        Err(RideError::RideNotRequested { .. }) => {
            println!("  Carol correctly rejected — ride already taken")
        }
        other => panic!("expected RideNotRequested, got {other:?}"),
    }

    println!("\n== Start and complete ride ==");
    service.start_ride(ride.id).expect("ride starts");
    let payment = service
        .complete_ride(ride.id, 22.0)
        .expect("ride completes");
    assert_eq!(payment.status, PaymentStatus::Completed);
    assert!(payment.amount > 0.0);
    println!("  fare: ${:.2}", payment.amount);
    assert_eq!(
        service.driver_status(bob.id).unwrap(),
        DriverStatus::Available
    );

    println!("\n== Cancel a fresh ride ==");
    let ride2 = service
        .request_ride(
            alice.id,
            alice.location,
            Location::new(37.8044, -122.2712),
            RideType::Regular,
        )
        .expect("ride requested");
    service
        .accept_ride(ride2.id, carol.id)
        .expect("carol accepts");
    service.cancel_ride(ride2.id).expect("ride cancelled");
    assert_eq!(
        service.driver_status(carol.id).unwrap(),
        DriverStatus::Available
    );

    match service.request_ride(
        uuid::Uuid::new_v4(),
        alice.location,
        alice.location,
        RideType::Regular,
    ) {
        Err(RideError::PassengerNotFound { .. }) => {
            println!("\nUnknown passenger correctly rejected")
        }
        other => panic!("expected PassengerNotFound, got {other:?}"),
    }
}

fn main() {
    run_demo();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sf() -> Location {
        Location::new(37.7749, -122.4194)
    }

    fn oakland() -> Location {
        Location::new(37.8044, -122.2712)
    }

    #[test]
    fn test_distance_km_symmetric() {
        let d1 = sf().distance_km(&oakland());
        let d2 = oakland().distance_km(&sf());
        assert!(d1 > 0.0);
        assert!((d1 - d2).abs() < 1e-9);
        assert_eq!(sf().distance_km(&sf()), 0.0);
    }

    #[test]
    fn test_fare_scales_with_ride_type() {
        let regular = RideService::calculate_fare(RideType::Regular, 10.0, 15.0);
        let premium = RideService::calculate_fare(RideType::Premium, 10.0, 15.0);
        assert!(premium > regular);
    }

    #[test]
    fn test_nearest_driver_match() {
        let service = RideService::new();
        let near = service.add_driver("Near", "1", "N1", Location::new(37.7750, -122.4180));
        let far = service.add_driver("Far", "2", "F1", Location::new(38.5, -121.5));
        assert_eq!(service.nearest_available_driver(&sf()), Some(near.id));
        let _ = far.id;
    }

    #[test]
    fn test_accept_ride_already_taken() {
        let service = RideService::new();
        let alice = service.add_passenger("Alice", "1", sf());
        let bob = service.add_driver("Bob", "2", "B1", sf());
        let carol = service.add_driver("Carol", "3", "C1", sf());
        let ride = service
            .request_ride(alice.id, sf(), oakland(), RideType::Regular)
            .unwrap();

        service.accept_ride(ride.id, bob.id).unwrap();
        let err = service.accept_ride(ride.id, carol.id).unwrap_err();
        assert!(matches!(err, RideError::RideNotRequested { .. }));
    }

    /// Requirement #8: many drivers race to accept the same ride. The ride
    /// and driver maps are locked together for the whole check-then-commit
    /// in `accept_ride`, so exactly one thread must win regardless of
    /// interleaving.
    #[test]
    fn test_concurrent_accept_one_winner() {
        let service = Arc::new(RideService::new());
        let alice = service.add_passenger("Alice", "1", sf());
        let driver_ids: Vec<_> = (0..16)
            .map(|i| {
                service
                    .add_driver(&format!("Driver{i}"), "x", "PLATE", sf())
                    .id
            })
            .collect();
        let ride = service
            .request_ride(alice.id, sf(), oakland(), RideType::Regular)
            .unwrap();

        let handles: Vec<_> = driver_ids
            .into_iter()
            .map(|driver_id| {
                let service = Arc::clone(&service);
                std::thread::spawn(move || service.accept_ride(ride.id, driver_id).is_ok())
            })
            .collect();

        let winners = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(winners, 1, "exactly one driver must win the race");
    }
}
