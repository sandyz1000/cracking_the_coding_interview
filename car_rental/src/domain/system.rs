use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::domain::accounts::{Address, Customer, User, UserRole};
use crate::domain::payments::{PaymentGateway, PaymentProcessor};
use crate::domain::reservations::{ReservationBook, ReservationManager, ReservationStatus};
use crate::domain::search::CarSearch;
use crate::domain::vehicles::{Vehicle, VehicleRegistry, VehicleSnapshot, VehicleStatus};
use crate::error::{CarError, CarResult};
use crate::locks::{rd, wr};

/// Composition root: every shared component is built once and handed out via
/// `Arc`. See DESIGN.md.
pub struct CarRentalSystem {
    pub vehicles: Arc<VehicleRegistry>,
    pub reservations: Arc<ReservationManager>,
    pub search: Arc<CarSearch>,
    pub payments: Arc<PaymentProcessor>,
    customers: RwLock<HashMap<u64, Customer>>,
    users: RwLock<HashMap<u64, User>>,
    locations: RwLock<HashMap<String, Address>>,
    next_id: AtomicU64,
}

impl CarRentalSystem {
    pub fn with_gateway(gateway: Box<dyn PaymentGateway>) -> Self {
        let vehicles: Arc<VehicleRegistry> = Arc::new(RwLock::new(HashMap::new()));
        let payments = Arc::new(PaymentProcessor::new(gateway));
        let book: Arc<ReservationBook> = Arc::new(RwLock::new(HashMap::new()));
        let reservations = Arc::new(ReservationManager::new(
            book.clone(),
            vehicles.clone(),
            payments.clone(),
        ));
        let search = Arc::new(CarSearch::new(vehicles.clone(), book));
        Self {
            vehicles,
            reservations,
            search,
            payments,
            customers: RwLock::new(HashMap::new()),
            users: RwLock::new(HashMap::new()),
            locations: RwLock::new(HashMap::new()),
            next_id: AtomicU64::new(0),
        }
    }

    pub fn register_customer(
        &self,
        name: &str,
        email: &str,
        phone: &str,
        license_number: &str,
    ) -> Customer {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let customer = Customer {
            id,
            name: name.to_string(),
            email: email.to_string(),
            phone: phone.to_string(),
            license_number: license_number.to_string(),
        };
        wr(&self.customers).insert(id, customer.clone());
        wr(&self.users).insert(id, User::new(id, &customer.name, UserRole::Customer));
        customer
    }

    pub fn user(&self, id: u64) -> Option<User> {
        rd(&self.users).get(&id).cloned()
    }

    pub fn customer(&self, id: u64) -> Option<Customer> {
        rd(&self.customers).get(&id).cloned()
    }

    pub fn add_new_location(&self, name: String, address: Address) {
        wr(&self.locations).insert(name, address);
    }

    pub fn add_vehicle(&self, vehicle: Vehicle) {
        wr(&self.vehicles).insert(vehicle.barcode.clone(), Arc::new(vehicle));
    }

    /// Taking a car out of service is an ops action, so it is manager-only
    /// like removal.
    pub fn set_vehicle_status(
        &self,
        barcode: &str,
        status: VehicleStatus,
        acting_user: &User,
    ) -> CarResult<()> {
        if !acting_user.is_manager() {
            return Err(CarError::PermissionDenied);
        }
        rd(&self.vehicles)
            .get(barcode)
            .ok_or_else(|| CarError::VehicleNotFound(barcode.to_string()))?
            .set_status(status);
        Ok(())
    }

    /// Only a manager may remove a vehicle, and only if it is not out on a
    /// rental and has no active reservation.
    pub fn remove_vehicle(&self, barcode: &str, acting_user: &User) -> CarResult<()> {
        if !acting_user.is_manager() {
            return Err(CarError::PermissionDenied);
        }
        let vehicle = rd(&self.vehicles)
            .get(barcode)
            .cloned()
            .ok_or_else(|| CarError::VehicleNotFound(barcode.to_string()))?;
        if vehicle.status() == VehicleStatus::Loaned {
            return Err(CarError::InvalidTransition(format!(
                "vehicle {barcode} is currently loaned"
            )));
        }
        {
            let reservations = rd(&self.reservations.book);
            if reservations.values().any(|r| {
                r.vehicle_barcode == barcode
                    && matches!(
                        r.status,
                        ReservationStatus::Pending | ReservationStatus::Confirmed
                    )
            }) {
                return Err(CarError::InvalidTransition(format!(
                    "vehicle {barcode} has an active reservation"
                )));
            }
        }
        wr(&self.vehicles).remove(barcode);
        Ok(())
    }

    pub fn vehicle_snapshot(&self, barcode: &str) -> Option<VehicleSnapshot> {
        rd(&self.vehicles).get(barcode).map(|v| v.snapshot())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permissions() {
        let system = crate::test_util::system();
        let customer = crate::test_util::customer(&system, "Alice");
        let manager = User::new(9, "Manager", UserRole::Manager);
        let vehicle = crate::test_util::vehicle("B-1");
        system.add_vehicle(vehicle);

        // Customers cannot remove vehicles; managers can.
        assert!(matches!(
            system.remove_vehicle("B-1", &customer),
            Err(CarError::PermissionDenied)
        ));
        assert!(system.remove_vehicle("B-1", &manager).is_ok());
    }
}
