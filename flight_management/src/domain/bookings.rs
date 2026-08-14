use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::domain::accounts::User;
use crate::domain::flights::{Flight, FlightRegistry, SeatType};
use crate::domain::payments::{Payment, PaymentMethod, PaymentProcessor};
use crate::error::{AmsError, AmsResult};
use crate::locks::{rd, wr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BookingStatus {
    Reserved,
    Confirmed,
    Cancelled,
    Completed,
}

#[derive(Clone, Debug)]
pub struct Baggage {
    pub pieces: u32,
    pub total_weight_kg: f64,
    pub checked: bool,
}

#[derive(Clone, Debug)]
pub struct Booking {
    pub booking_number: u64,
    pub flight_number: String,
    pub passenger_id: u64,
    pub seat_number: String,
    pub seat_type: SeatType,
    pub price: f64,
    pub refunded: f64,
    pub status: BookingStatus,
    pub baggage: Option<Baggage>,
}

#[derive(Clone, Debug)]
pub struct CancelResult {
    pub booking_number: u64,
    pub refunded: f64,
}

#[derive(Clone, Debug)]
pub struct ChangeResult {
    pub booking: Booking,
    pub fare_difference: f64,
}

pub struct BookingManager {
    next_booking: AtomicU64,
    bookings: RwLock<HashMap<u64, Booking>>,
    flights: Arc<FlightRegistry>,
    payments: Arc<PaymentProcessor>,
}

impl BookingManager {
    pub fn new(flights: Arc<FlightRegistry>, payments: Arc<PaymentProcessor>) -> Self {
        Self {
            next_booking: AtomicU64::new(0),
            bookings: RwLock::new(HashMap::new()),
            flights,
            payments,
        }
    }

    fn flight(&self, flight_number: &str) -> AmsResult<Arc<Flight>> {
        rd(&self.flights)
            .get(flight_number)
            .cloned()
            .ok_or_else(|| AmsError::FlightNotFound(flight_number.to_string()))
    }

    fn release_seat_for(&self, flight_number: &str, seat_number: &str) {
        if let Some(flight) = rd(&self.flights).get(flight_number).cloned() {
            flight.release(seat_number);
        }
    }

    /// Book a seat and pay as one transaction:
    ///   1. RESERVE the seat under the inventory lock.
    ///   2. PAY through the gateway outside the lock — gateway latency must
    ///      not block other passengers.
    ///   3. COMMIT (confirm) or ROLL BACK (release) on payment failure.
    ///
    /// Never hold two locks at once; the reservation under a write lock is
    /// what makes double-booking impossible.
    pub fn book(
        &self,
        user: &User,
        flight_number: &str,
        seat_number: &str,
        method: PaymentMethod,
    ) -> AmsResult<(Booking, Payment)> {
        let flight = self.flight(flight_number)?;
        let booking_number = self.next_booking.fetch_add(1, Ordering::Relaxed) + 1;

        let (seat_type, price) = flight.reserve(seat_number, user.id)?;

        let payment = match self.payments.process(booking_number, price, method) {
            Ok(payment) => payment,
            Err(err) => {
                flight.release(seat_number); // rollback the reservation
                return Err(err);
            }
        };

        flight.confirm(seat_number);
        let booking = Booking {
            booking_number,
            flight_number: flight.flight_number.clone(),
            passenger_id: user.id,
            seat_number: seat_number.to_string(),
            seat_type,
            price,
            refunded: 0.0,
            status: BookingStatus::Confirmed,
            baggage: None,
        };
        wr(&self.bookings).insert(booking_number, booking.clone());
        Ok((booking, payment))
    }

    /// Cancel: validate, mark Cancelled, release the seat, then refund.
    /// Booking state is committed before the refund so a refund failure can
    /// never leave a seat locked.
    pub fn cancel(&self, booking_number: u64, acting_user: &User) -> AmsResult<CancelResult> {
        let (flight_number, seat_number, remaining_refund) = {
            let mut bookings = wr(&self.bookings);
            let booking = bookings
                .get_mut(&booking_number)
                .ok_or(AmsError::BookingNotFound(booking_number))?;
            if acting_user.role == crate::domain::accounts::UserRole::Passenger
                && booking.passenger_id != acting_user.id
            {
                return Err(AmsError::PermissionDenied);
            }
            if matches!(
                booking.status,
                BookingStatus::Cancelled | BookingStatus::Completed
            ) {
                return Err(AmsError::InvalidTransition(format!(
                    "booking {booking_number} is {:?} and cannot be cancelled",
                    booking.status
                )));
            }
            booking.status = BookingStatus::Cancelled;
            (
                booking.flight_number.clone(),
                booking.seat_number.clone(),
                booking.price - booking.refunded,
            )
        };

        self.release_seat_for(&flight_number, &seat_number);
        let refunded = if remaining_refund > 0.0 {
            self.payments.refund(booking_number, remaining_refund)?
        } else {
            0.0
        };
        Ok(CancelResult {
            booking_number,
            refunded,
        })
    }

    /// Move a confirmed booking to another flight/seat. The new seat is
    /// reserved first so the passenger is never left seatless; the fare
    /// difference is settled (extra charge or refund) before committing.
    pub fn change_flight(
        &self,
        booking_number: u64,
        new_flight_number: &str,
        seat_number: &str,
        method: PaymentMethod,
        acting_user: &User,
    ) -> AmsResult<ChangeResult> {
        let old = {
            let bookings = rd(&self.bookings);
            bookings
                .get(&booking_number)
                .cloned()
                .ok_or(AmsError::BookingNotFound(booking_number))?
        };
        if acting_user.role == crate::domain::accounts::UserRole::Passenger
            && old.passenger_id != acting_user.id
        {
            return Err(AmsError::PermissionDenied);
        }
        if old.status != BookingStatus::Confirmed {
            return Err(AmsError::InvalidTransition(format!(
                "booking {booking_number} is {:?}; only confirmed bookings can change",
                old.status
            )));
        }

        let new_flight = self.flight(new_flight_number)?;
        let (new_seat_type, new_price) = new_flight.reserve(seat_number, old.passenger_id)?;

        let difference = new_price - old.price;
        let refunded = if difference < 0.0 {
            match self.payments.refund(booking_number, -difference) {
                Ok(refunded) => refunded,
                Err(err) => {
                    new_flight.release(seat_number); // rollback the reservation
                    return Err(err);
                }
            }
        } else if difference > 0.0 {
            if let Err(err) = self.payments.process(booking_number, difference, method) {
                new_flight.release(seat_number); // rollback the reservation
                return Err(err);
            }
            0.0
        } else {
            0.0
        };

        new_flight.confirm(seat_number);
        self.release_seat_for(&old.flight_number, &old.seat_number);
        let updated = Booking {
            flight_number: new_flight.flight_number.clone(),
            seat_number: seat_number.to_string(),
            seat_type: new_seat_type,
            price: new_price,
            refunded: old.refunded + refunded,
            ..old
        };
        wr(&self.bookings).insert(booking_number, updated.clone());
        Ok(ChangeResult {
            booking: updated,
            fare_difference: difference,
        })
    }

    pub fn set_baggage(
        &self,
        booking_number: u64,
        baggage: Option<Baggage>,
        acting_user: &User,
    ) -> AmsResult<()> {
        let mut bookings = wr(&self.bookings);
        let booking = bookings
            .get_mut(&booking_number)
            .ok_or(AmsError::BookingNotFound(booking_number))?;
        if acting_user.role == crate::domain::accounts::UserRole::Passenger
            && booking.passenger_id != acting_user.id
        {
            return Err(AmsError::PermissionDenied);
        }
        booking.baggage = baggage;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FlightSpec;
    use crate::domain::accounts::UserRole;
    use crate::domain::flights::Aircraft;
    use crate::domain::payments::PaymentStatus;
    use crate::test_util;
    use crate::time::{Date, Time};
    use std::thread;

    #[test]
    fn test_booking_flow() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let user = test_util::passenger(&ams, "Asha");

        let (booking, payment) = ams
            .bookings
            .book(&user, &flight.flight_number, "1A", PaymentMethod::Card)
            .unwrap();
        assert_eq!(booking.status, BookingStatus::Confirmed);
        assert_eq!(payment.status, PaymentStatus::Completed);
        assert_eq!(flight.available_seats(), 35);
    }

    #[test]
    fn test_seat_conflict() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let user = test_util::passenger(&ams, "Asha");

        ams.bookings
            .book(&user, &flight.flight_number, "3C", PaymentMethod::Card)
            .unwrap();
        let err = ams
            .bookings
            .book(&user, &flight.flight_number, "3C", PaymentMethod::Upi)
            .unwrap_err();
        assert!(matches!(err, AmsError::SeatNotAvailable(_)));
    }

    #[test]
    fn test_seat_race() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let user = test_util::passenger(&ams, "Asha");

        let handles: Vec<_> = (0..64)
            .map(|_| {
                let ams = ams.clone();
                let user = user.clone();
                let flight = flight.clone();
                thread::spawn(move || {
                    ams.bookings
                        .book(&user, &flight.flight_number, "3C", PaymentMethod::Card)
                        .map(|_| ())
                })
            })
            .collect();

        let winners = handles
            .into_iter()
            .map(|handle| handle.join().unwrap().is_ok())
            .filter(|ok| *ok)
            .count();
        assert_eq!(winners, 1, "exactly one concurrent booking must win");
    }

    #[test]
    fn test_full_inventory() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let user = test_util::passenger(&ams, "Asha");
        let seat_numbers: Vec<String> = flight
            .seat_map()
            .into_iter()
            .map(|seat| seat.number)
            .collect();
        assert_eq!(seat_numbers.len(), 36);

        let handles: Vec<_> = seat_numbers
            .into_iter()
            .map(|seat_number| {
                let ams = ams.clone();
                let user = user.clone();
                let flight = flight.clone();
                thread::spawn(move || {
                    ams.bookings.book(
                        &user,
                        &flight.flight_number,
                        &seat_number,
                        PaymentMethod::Card,
                    )
                })
            })
            .collect();

        let failures: Vec<_> = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap().err())
            .collect();
        assert!(
            failures.is_empty(),
            "all 36 seats should book: {failures:?}"
        );
        assert_eq!(flight.available_seats(), 0);
    }

    #[test]
    fn test_cancel_refund() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let user = test_util::passenger(&ams, "Asha");

        let (booking, payment) = ams
            .bookings
            .book(&user, &flight.flight_number, "4D", PaymentMethod::Card)
            .unwrap();
        assert_eq!(booking.price, 100.0);

        let cancel = ams.bookings.cancel(booking.booking_number, &user).unwrap();
        assert_eq!(cancel.refunded, booking.price);
        let seats = flight.seat_map();
        let seat = seats.iter().find(|seat| seat.number == "4D").unwrap();
        assert_eq!(seat.status, crate::domain::flights::SeatStatus::Available);
        assert_eq!(seat.holder, None);

        let payments = rd(&ams.payments.payments);
        let stored = payments.get(&payment.payment_id).unwrap();
        assert_eq!(stored.status, PaymentStatus::Refunded);
    }

    #[test]
    fn test_baggage_access() {
        let ams = test_util::system();
        let flight = test_util::flight(&ams);
        let owner = test_util::passenger(&ams, "Owner");
        let stranger = test_util::passenger(&ams, "Stranger");

        let (booking, _) = ams
            .bookings
            .book(&owner, &flight.flight_number, "1A", PaymentMethod::Card)
            .unwrap();
        let baggage = Some(Baggage {
            pieces: 1,
            total_weight_kg: 18.5,
            checked: true,
        });
        ams.bookings
            .set_baggage(booking.booking_number, baggage.clone(), &owner)
            .unwrap();

        let err = ams
            .bookings
            .set_baggage(booking.booking_number, baggage, &stranger)
            .unwrap_err();
        assert!(matches!(err, AmsError::PermissionDenied));
    }

    #[test]
    fn test_change_upgrades() {
        let ams = test_util::system();
        let morning = test_util::flight(&ams);
        let admin = User::new(1, "Admin", UserRole::Admin);
        let evening = ams
            .schedule_flight(
                &admin,
                Arc::new(Aircraft {
                    tail_number: "T2".into(),
                    model: "BiggerJet".into(),
                    total_seats: 36,
                }),
                FlightSpec {
                    source: "DEL".into(),
                    destination: "BOM".into(),
                    date: Date::new(2026, 1, 1),
                    departure: Time::new(17, 0),
                    arrival: Time::new(19, 0),
                },
                150.0,
            )
            .unwrap();
        let user = test_util::passenger(&ams, "Asha");

        let (booking, _) = ams
            .bookings
            .book(&user, &morning.flight_number, "2A", PaymentMethod::Card)
            .unwrap();
        let changed = ams
            .bookings
            .change_flight(
                booking.booking_number,
                &evening.flight_number,
                "1A",
                PaymentMethod::Wallet,
                &user,
            )
            .unwrap();

        assert_eq!(changed.booking.flight_number, evening.flight_number);
        assert_eq!(changed.booking.seat_number, "1A");
        assert!((changed.fare_difference - 305.0).abs() < 0.001);

        let morning_seats = morning.seat_map();
        let evening_seats = evening.seat_map();
        let old_seat = morning_seats
            .iter()
            .find(|seat| seat.number == "2A")
            .unwrap();
        assert_eq!(
            old_seat.status,
            crate::domain::flights::SeatStatus::Available
        );
        let new_seat = evening_seats
            .iter()
            .find(|seat| seat.number == "1A")
            .unwrap();
        assert_eq!(new_seat.status, crate::domain::flights::SeatStatus::Booked);
        assert_eq!(new_seat.holder, Some(user.id));
    }

    #[test]
    fn test_change_downgrades() {
        let ams = test_util::system();
        let morning = test_util::flight(&ams);
        let admin = User::new(1, "Admin", UserRole::Admin);
        let cheaper = ams
            .schedule_flight(
                &admin,
                Arc::new(Aircraft {
                    tail_number: "T3".into(),
                    model: "NarrowJet".into(),
                    total_seats: 36,
                }),
                FlightSpec {
                    source: "DEL".into(),
                    destination: "BOM".into(),
                    date: Date::new(2026, 1, 1),
                    departure: Time::new(20, 0),
                    arrival: Time::new(22, 0),
                },
                60.0,
            )
            .unwrap();
        let user = test_util::passenger(&ams, "Asha");

        let (booking, payment) = ams
            .bookings
            .book(&user, &morning.flight_number, "2A", PaymentMethod::Card)
            .unwrap();
        let changed = ams
            .bookings
            .change_flight(
                booking.booking_number,
                &cheaper.flight_number,
                "5B",
                PaymentMethod::Card,
                &user,
            )
            .unwrap();

        assert!(changed.fare_difference < 0.0);
        assert!((changed.fare_difference - (-160.0)).abs() < 0.001);

        let payments = rd(&ams.payments.payments);
        let stored = payments.get(&payment.payment_id).unwrap();
        assert_eq!(stored.status, PaymentStatus::PartiallyRefunded);
        assert!((stored.refunded - 160.0).abs() < 0.001);
    }
}
