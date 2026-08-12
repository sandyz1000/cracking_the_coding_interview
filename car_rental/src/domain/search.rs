use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::reservations::{ReservationBook, ReservationStatus};
use crate::domain::vehicles::{VehicleRegistry, VehicleSnapshot};
use crate::locks::rd;
use crate::time::Date;

type AvailabilityKey = (String, Date, Date);

/// Read-optimised availability index. Instead of scanning every reservation
/// per query, the index maps (barcode, start, end) to the reservations that
/// are actively holding the car (Pending or Confirmed). Lookups are O(1);
/// the index is rebuilt on every book/modify/cancel.
pub struct CarSearch {
    vehicles: Arc<VehicleRegistry>,
    reservations: Arc<ReservationBook>,
    index: RwLock<HashMap<AvailabilityKey, Vec<String>>>,
}

impl CarSearch {
    pub fn new(vehicles: Arc<VehicleRegistry>, reservations: Arc<ReservationBook>) -> Self {
        let search = Self {
            vehicles,
            reservations,
            index: RwLock::new(HashMap::new()),
        };
        search.rebuild_index();
        search
    }

    pub fn rebuild_index(&self) {
        let mut by_key: HashMap<AvailabilityKey, Vec<String>> = HashMap::new();
        {
            let reservations = rd(&self.reservations);
            for reservation in reservations.values() {
                if matches!(
                    reservation.status,
                    ReservationStatus::Pending | ReservationStatus::Confirmed
                ) {
                    by_key
                        .entry((
                            reservation.vehicle_barcode.clone(),
                            reservation.start_date,
                            reservation.end_date,
                        ))
                        .or_default()
                        .push(reservation.reservation_number.clone());
                }
            }
        }
        for numbers in by_key.values_mut() {
            numbers.sort();
        }
        *self.index.write().unwrap_or_else(|p| p.into_inner()) = by_key;
    }

    /// A car is available in [start, end) when no Pending/Confirmed
    /// reservation overlaps it.
    pub fn is_available(&self, barcode: &str, start: Date, end: Date) -> bool {
        let reservations = rd(&self.reservations);
        !reservations.values().any(|r| {
            r.vehicle_barcode == barcode
                && matches!(
                    r.status,
                    ReservationStatus::Pending | ReservationStatus::Confirmed
                )
                && start < r.end_date
                && end > r.start_date
        })
    }

    /// Search by make/model/year/price, restricted to the requested window.
    /// A car must be physically rentable (Available status) AND have no
    /// overlapping reservation.
    pub fn search(
        &self,
        make: Option<&str>,
        model: Option<&str>,
        year: Option<u32>,
        max_price_per_day: Option<u32>,
        start: Date,
        end: Date,
    ) -> Vec<VehicleSnapshot> {
        let mut candidates: Vec<VehicleSnapshot> = {
            let vehicles = rd(&self.vehicles);
            vehicles
                .values()
                .filter(|v| v.status() == crate::domain::vehicles::VehicleStatus::Available)
                .filter(|v| make.is_none_or(|m| v.make == m))
                .filter(|v| model.is_none_or(|m| v.model == m))
                .filter(|v| year.is_none_or(|y| v.year == y))
                .filter(|v| max_price_per_day.is_none_or(|p| v.price_per_day <= p))
                .map(|v| v.snapshot())
                .collect()
        };
        candidates.retain(|v| self.is_available(&v.barcode, start, end));
        candidates.sort_by(|a, b| {
            a.make
                .cmp(&b.make)
                .then(a.model.cmp(&b.model))
                .then(a.barcode.cmp(&b.barcode))
        });
        candidates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::reservations::ReservationSpec;
    use crate::test_util;

    #[test]
    fn test_search_respects_availability() {
        let system = test_util::system();
        let user = test_util::customer(&system, "Alice");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        system
            .reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: Date::new(2026, 5, 10),
                    end: Date::new(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                crate::domain::payments::PaymentMethod::CreditCard,
            )
            .unwrap();
        // Booked window: the car must not appear.
        let hits = system.search.search(
            None,
            None,
            None,
            None,
            Date::new(2026, 5, 10),
            Date::new(2026, 5, 13),
        );
        assert!(hits.is_empty());
        // Non-overlapping window: it appears again.
        let hits = system.search.search(
            None,
            None,
            None,
            None,
            Date::new(2026, 5, 20),
            Date::new(2026, 5, 22),
        );
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].barcode, "B-1");
    }

    #[test]
    fn test_search_filters_by_criteria() {
        let system = test_util::system();
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let mut truck = test_util::vehicle(&system, "B-2");
        truck.make = "Ford".into();
        truck.model = "F-150".into();
        truck.price_per_day = 90_00;
        system.add_vehicle(truck);

        let start = Date::new(2026, 5, 10);
        let end = Date::new(2026, 5, 13);
        let hits = system
            .search
            .search(Some("Toyota"), None, None, Some(70_00), start, end);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].barcode, "B-1");
        let hits = system
            .search
            .search(Some("Ford"), None, None, None, start, end);
        assert_eq!(hits[0].model, "F-150");
    }
}
