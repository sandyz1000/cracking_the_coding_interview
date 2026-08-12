use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::domain::accounts::{User, UserRole};
use crate::domain::payments::{Payment, PaymentMethod, PaymentProcessor};
use crate::domain::vehicles::{Vehicle, VehicleRegistry, VehicleStatus};
use crate::error::{CarError, CarResult};
use crate::locks::{rd, wr};
use crate::time::Date;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ReservationStatus {
    /// Held while payment is being processed (book/modify in flight).
    Pending,
    Confirmed,
    Active,
    Completed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct Reservation {
    pub reservation_number: String,
    pub creation_date: Date,
    pub status: ReservationStatus,
    pub start_date: Date,
    pub end_date: Date, // exclusive
    pub pickup_location: String,
    pub return_location: String,
    pub customer_id: u64,
    pub vehicle_barcode: String,
    pub total_price: u32, // in cents
    pub refunded: u32,
}

impl Reservation {
    pub fn nights(&self) -> i64 {
        self.end_date.nights_from(self.start_date)
    }
}

pub type ReservationBook = RwLock<HashMap<String, Reservation>>;

/// The booking engine. Two-phase transactions (reserve → pay → commit) make
/// double-booking impossible; settlement (charge/refund) never runs while a
/// lock is held. See `DESIGN.md`.
/// Bundled booking window input for `book`, keeping the call under the
/// seven-argument limit.
#[derive(Clone, Copy, Debug)]
pub struct ReservationSpec {
    pub start: Date,
    pub end: Date, // exclusive
    pub pickup: &'static str,
    pub dropoff: &'static str,
}

pub struct ReservationManager {
    next_reservation: AtomicU64,
    pub book: Arc<ReservationBook>,
    vehicles: Arc<VehicleRegistry>,
    payments: Arc<PaymentProcessor>,
}

impl ReservationManager {
    pub fn new(
        book: Arc<ReservationBook>,
        vehicles: Arc<VehicleRegistry>,
        payments: Arc<PaymentProcessor>,
    ) -> Self {
        Self {
            next_reservation: AtomicU64::new(0),
            book,
            vehicles,
            payments,
        }
    }

    fn vehicle(&self, barcode: &str) -> CarResult<Arc<Vehicle>> {
        rd(&self.vehicles)
            .get(barcode)
            .cloned()
            .ok_or_else(|| CarError::VehicleNotFound(barcode.to_string()))
    }

    fn assert_access(acting: &User, owner_id: u64) -> CarResult<()> {
        if acting.role == UserRole::Customer && acting.id != owner_id {
            return Err(CarError::PermissionDenied);
        }
        Ok(())
    }

    /// True when a reservation on the car would collide with another
    /// reservation in [start, end). `exclude` is the reservation being
    /// moved; Pending blocks too, because a pending reservation is holding
    /// the car while its payment is in flight.
    fn conflicts(
        &self,
        vehicle_barcode: &str,
        start: Date,
        end: Date,
        exclude: Option<&str>,
    ) -> bool {
        let reservations = rd(&self.book);
        reservations.values().any(|r| {
            r.vehicle_barcode == vehicle_barcode
                && matches!(
                    r.status,
                    ReservationStatus::Pending | ReservationStatus::Confirmed
                )
                && exclude != Some(r.reservation_number.as_str())
                && start < r.end_date
                && end > r.start_date
        })
    }

    /// Book a car and pay as one transaction:
    ///   1. RESERVE: write the Pending reservation under the reservations
    ///      lock (this is what makes double-booking impossible).
    ///   2. PAY through the gateway outside the lock — gateway latency must
    ///      not block other bookings.
    ///   3. COMMIT (Confirmed) or roll back on payment failure.
    pub fn book(
        &self,
        customer_id: u64,
        vehicle_barcode: &str,
        spec: ReservationSpec,
        method: PaymentMethod,
    ) -> CarResult<(Reservation, Payment)> {
        if spec.end <= spec.start {
            return Err(CarError::InvalidDates(
                "end date must be after start date".into(),
            ));
        }
        let vehicle = self.vehicle(vehicle_barcode)?;
        if vehicle.status() != VehicleStatus::Available {
            return Err(CarError::VehicleNotAvailable(format!(
                "vehicle {vehicle_barcode} is {:?}",
                vehicle.status()
            )));
        }
        if self.conflicts(vehicle_barcode, spec.start, spec.end, None) {
            return Err(CarError::VehicleNotAvailable(format!(
                "vehicle {vehicle_barcode} is already reserved for those dates"
            )));
        }

        let reservation_number = format!(
            "R-{:05}",
            self.next_reservation.fetch_add(1, Ordering::Relaxed) + 1
        );
        let total = (vehicle.price_per_day as i64 * spec.end.nights_from(spec.start))
            .try_into()
            .map_err(|_| CarError::InvalidDates("reservation total overflow".into()))?;

        let mut reservation = Reservation {
            reservation_number: reservation_number.clone(),
            creation_date: Date::new(2026, 1, 1),
            status: ReservationStatus::Pending,
            start_date: spec.start,
            end_date: spec.end,
            pickup_location: spec.pickup.to_string(),
            return_location: spec.dropoff.to_string(),
            customer_id,
            vehicle_barcode: vehicle_barcode.to_string(),
            total_price: total,
            refunded: 0,
        };
        wr(&self.book).insert(reservation_number.clone(), reservation.clone());

        match self.payments.process(&reservation_number, total, method) {
            Ok(payment) => {
                reservation.status = ReservationStatus::Confirmed;
                wr(&self.book).insert(reservation_number.clone(), reservation.clone());
                Ok((reservation, payment))
            }
            Err(err) => {
                reservation.status = ReservationStatus::Cancelled; // audit trail
                wr(&self.book).insert(reservation_number.clone(), reservation.clone());
                Err(err)
            }
        }
    }

    /// The rental begins: the car leaves the lot and becomes Loaned.
    pub fn start_rental(&self, reservation_number: &str, acting_user: &User) -> CarResult<()> {
        let vehicle_barcode = {
            let mut reservations = wr(&self.book);
            let reservation = reservations
                .get_mut(reservation_number)
                .ok_or_else(|| CarError::ReservationNotFound(reservation_number.into()))?;
            Self::assert_access(acting_user, reservation.customer_id)?;
            if reservation.status != ReservationStatus::Confirmed {
                return Err(CarError::InvalidTransition(format!(
                    "cannot start a {:?} reservation",
                    reservation.status
                )));
            }
            reservation.status = ReservationStatus::Active;
            reservation.vehicle_barcode.clone()
        };
        if let Ok(vehicle) = self.vehicle(&vehicle_barcode) {
            vehicle.set_status(VehicleStatus::Loaned);
        }
        Ok(())
    }

    /// The rental ends: mark Completed and return the car to the pool.
    pub fn complete_rental(&self, reservation_number: &str, acting_user: &User) -> CarResult<()> {
        let vehicle_barcode = {
            let mut reservations = wr(&self.book);
            let reservation = reservations
                .get_mut(reservation_number)
                .ok_or_else(|| CarError::ReservationNotFound(reservation_number.into()))?;
            Self::assert_access(acting_user, reservation.customer_id)?;
            if reservation.status != ReservationStatus::Active {
                return Err(CarError::InvalidTransition(format!(
                    "cannot complete a {:?} reservation",
                    reservation.status
                )));
            }
            reservation.status = ReservationStatus::Completed;
            reservation.vehicle_barcode.clone()
        };
        if let Ok(vehicle) = self.vehicle(&vehicle_barcode) {
            // A Lost vehicle must not silently return to the pool.
            if vehicle.status() == VehicleStatus::Loaned {
                vehicle.set_status(VehicleStatus::Available);
            }
        }
        Ok(())
    }

    /// Cancel: only the owner (or staff), only a Confirmed reservation, only
    /// before the rental starts. The car is freed immediately (status
    /// Cancelled), then the refund is settled outside the lock.
    pub fn cancel(&self, reservation_number: &str, acting_user: &User) -> CarResult<u32> {
        let remaining_refund = {
            let mut reservations = wr(&self.book);
            let reservation = reservations
                .get_mut(reservation_number)
                .ok_or_else(|| CarError::ReservationNotFound(reservation_number.into()))?;
            Self::assert_access(acting_user, reservation.customer_id)?;
            if reservation.status != ReservationStatus::Confirmed {
                return Err(CarError::InvalidTransition(format!(
                    "cannot cancel a {:?} reservation",
                    reservation.status
                )));
            }
            reservation.status = ReservationStatus::Cancelled;
            reservation.total_price - reservation.refunded
        };
        if remaining_refund > 0 {
            self.payments
                .refund(reservation_number, remaining_refund)
                .map_err(|err| CarError::RefundFailed(err.to_string()))
        } else {
            Ok(0)
        }
    }

    /// Move a confirmed reservation to a new date range. The new range is
    /// written as Pending first (so the car is held during settlement), the
    /// price delta is settled outside the lock, and only then is the
    /// reservation re-committed as Confirmed. On settlement failure the
    /// original dates are restored.
    pub fn modify(
        &self,
        reservation_number: &str,
        acting_user: &User,
        new_start: Date,
        new_end: Date,
        method: PaymentMethod,
    ) -> CarResult<u32> {
        if new_end <= new_start {
            return Err(CarError::InvalidDates(
                "end date must be after start date".into(),
            ));
        }
        let (original, price_per_day) = {
            let reservations = rd(&self.book);
            let reservation = reservations
                .get(reservation_number)
                .ok_or_else(|| CarError::ReservationNotFound(reservation_number.into()))?
                .clone();
            Self::assert_access(acting_user, reservation.customer_id)?;
            if reservation.status != ReservationStatus::Confirmed {
                return Err(CarError::InvalidTransition(format!(
                    "cannot modify a {:?} reservation",
                    reservation.status
                )));
            }
            let vehicle = self.vehicle(&reservation.vehicle_barcode)?;
            (reservation, vehicle.price_per_day)
        };

        if self.conflicts(
            &original.vehicle_barcode,
            new_start,
            new_end,
            Some(reservation_number),
        ) {
            return Err(CarError::VehicleNotAvailable(
                "new dates overlap another reservation".into(),
            ));
        }

        let new_total: u32 = (price_per_day as i64 * new_end.nights_from(new_start))
            .try_into()
            .map_err(|_| CarError::InvalidDates("reservation total overflow".into()))?;
        let delta = new_total as i64 - original.total_price as i64;

        // Hold the car while settlement is in flight.
        let mut pending = original.clone();
        pending.status = ReservationStatus::Pending;
        pending.start_date = new_start;
        pending.end_date = new_end;
        pending.total_price = new_total;
        wr(&self.book).insert(reservation_number.to_string(), pending.clone());

        if delta > 0 {
            if let Err(err) = self
                .payments
                .process(reservation_number, delta as u32, method)
            {
                wr(&self.book).insert(reservation_number.to_string(), original);
                return Err(err);
            }
        } else if delta < 0 {
            match self.payments.refund(reservation_number, (-delta) as u32) {
                Ok(refunded) => pending.refunded += refunded,
                Err(err) => {
                    wr(&self.book).insert(reservation_number.to_string(), original);
                    return Err(CarError::RefundFailed(err.to_string()));
                }
            }
        }

        pending.status = ReservationStatus::Confirmed;
        wr(&self.book).insert(reservation_number.to_string(), pending);
        Ok(delta.unsigned_abs() as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::accounts::UserRole;
    use crate::domain::payments::PaymentStatus;
    use crate::locks::rd;
    use crate::test_util;
    use std::thread;

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::new(y as u32, m, day)
    }

    #[test]
    fn test_booking_flow() {
        let system = test_util::system();
        let user = test_util::customer(&system, "Alice");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));

        let (reservation, payment) = system
            .reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 10),
                    end: d(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        assert_eq!(reservation.status, ReservationStatus::Confirmed);
        assert_eq!(reservation.total_price, 15_000);
        assert_eq!(payment.status, PaymentStatus::Completed);
    }

    #[test]
    fn test_overlap_rejected() {
        let system = test_util::system();
        let user = test_util::customer(&system, "Alice");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let reservations = &system.reservations;

        reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 10),
                    end: d(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        // Identical window and an overlapping window both must fail.
        assert!(
            reservations
                .book(
                    user.id,
                    "B-1",
                    ReservationSpec {
                        start: d(2026, 5, 10),
                        end: d(2026, 5, 13),
                        pickup: "NYC",
                        dropoff: "NYC"
                    },
                    PaymentMethod::CreditCard,
                )
                .is_err()
        );
        assert!(
            reservations
                .book(
                    user.id,
                    "B-1",
                    ReservationSpec {
                        start: d(2026, 5, 12),
                        end: d(2026, 5, 15),
                        pickup: "NYC",
                        dropoff: "NYC"
                    },
                    PaymentMethod::CreditCard,
                )
                .is_err()
        );
        // Adjacent window (starts the day after) is fine.
        assert!(
            reservations
                .book(
                    user.id,
                    "B-1",
                    ReservationSpec {
                        start: d(2026, 5, 13),
                        end: d(2026, 5, 15),
                        pickup: "NYC",
                        dropoff: "NYC"
                    },
                    PaymentMethod::CreditCard,
                )
                .is_ok()
        );
    }

    #[test]
    fn test_concurrent_booking_race() {
        let system = test_util::system();
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let system = system.clone();
                thread::spawn(move || {
                    system
                        .reservations
                        .book(
                            i as u64 + 1,
                            "B-1",
                            ReservationSpec {
                                start: d(2026, 5, 10),
                                end: d(2026, 5, 13),
                                pickup: "NYC",
                                dropoff: "NYC",
                            },
                            PaymentMethod::CreditCard,
                        )
                        .is_ok()
                })
            })
            .collect();
        let winners = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|ok| *ok)
            .count();
        assert_eq!(winners, 1, "exactly one concurrent booking must win");
    }

    #[test]
    fn test_cancel_refunds_and_frees() {
        let system = test_util::system();
        let user = test_util::customer(&system, "Alice");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let reservations = &system.reservations;

        let (reservation, payment) = reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 10),
                    end: d(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        assert_eq!(reservation.total_price, 15_000);

        let refunded = reservations
            .cancel(&reservation.reservation_number, &user)
            .unwrap();
        assert_eq!(refunded, reservation.total_price);
        let payments = rd(&system.payments.payments);
        let stored = payments.get(&payment.payment_id).unwrap();
        assert_eq!(stored.status, PaymentStatus::Refunded);
        drop(payments);

        // The car is bookable again for the same dates.
        assert!(
            reservations
                .book(
                    user.id,
                    "B-1",
                    ReservationSpec {
                        start: d(2026, 5, 10),
                        end: d(2026, 5, 13),
                        pickup: "NYC",
                        dropoff: "NYC"
                    },
                    PaymentMethod::CreditCard,
                )
                .is_ok()
        );
    }

    #[test]
    fn test_cancel_wrong_customer() {
        let system = test_util::system();
        let owner = test_util::customer(&system, "Owner");
        let stranger = test_util::customer(&system, "Stranger");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let reservations = &system.reservations;

        let (reservation, _) = reservations
            .book(
                owner.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 10),
                    end: d(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        assert!(
            reservations
                .cancel(&reservation.reservation_number, &stranger)
                .is_err()
        );
        // Staff may cancel on the customer's behalf.
        let staff = User::new(99, "Receptionist", UserRole::Receptionist);
        assert!(
            reservations
                .cancel(&reservation.reservation_number, &staff)
                .is_ok()
        );
    }

    #[test]
    fn test_modify_rejects_overlap_but_not_self() {
        let system = test_util::system();
        let user = test_util::customer(&system, "Alice");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let reservations = &system.reservations;

        let (reservation, _) = reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 10),
                    end: d(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        // Extending over the reservation's own dates is a legitimate
        // modification and must not be rejected as a self-conflict.
        let delta = reservations
            .modify(
                &reservation.reservation_number,
                &user,
                d(2026, 5, 10),
                d(2026, 5, 15),
                PaymentMethod::CreditCard,
            )
            .unwrap();
        assert_eq!(delta, 10_000);

        // A second reservation holds 20th–21st; moving into it must fail.
        reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 20),
                    end: d(2026, 5, 22),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        assert!(
            reservations
                .modify(
                    &reservation.reservation_number,
                    &user,
                    d(2026, 5, 20),
                    d(2026, 5, 22),
                    PaymentMethod::CreditCard,
                )
                .is_err()
        );
    }

    #[test]
    fn test_lifecycle() {
        let system = test_util::system();
        let user = test_util::customer(&system, "Alice");
        system.add_vehicle(test_util::vehicle(&system, "B-1"));
        let reservations = &system.reservations;

        let (reservation, _) = reservations
            .book(
                user.id,
                "B-1",
                ReservationSpec {
                    start: d(2026, 5, 10),
                    end: d(2026, 5, 13),
                    pickup: "NYC",
                    dropoff: "NYC",
                },
                PaymentMethod::CreditCard,
            )
            .unwrap();
        reservations
            .start_rental(&reservation.reservation_number, &user)
            .unwrap();
        reservations
            .complete_rental(&reservation.reservation_number, &user)
            .unwrap();
        let stored = rd(&system.reservations.book)
            .get(&reservation.reservation_number)
            .unwrap()
            .clone();
        assert_eq!(stored.status, ReservationStatus::Completed);
    }
}
