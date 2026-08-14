//! Concert ticket booking system. Concurrency: each concert keeps its seats
//! behind an `RwLock`; `book_tickets` verifies and claims every requested seat
//! inside one write-lock critical section, so two requests can never claim
//! the same seat. See the design notes in the problem statement at the top of
//! this file for the DB-transaction equivalent.

/*
Designing a Concert Ticket Booking System

### Requirements

- The concert ticket booking system should allow users to view available concerts and their seating arrangements.
- Users should be able to search for concerts based on various criteria such as artist, venue, date, and time.
- Users should be able to select seats and purchase tickets for a specific concert.
- The system should handle concurrent booking requests to avoid double-booking of seats.
- The system should ensure fair booking opportunities for all users.
- The system should handle payment processing securely.
- The system should generate booking confirmations and send them to users via email or SMS.
- The system should provide a waiting list functionality for sold-out concerts.

### Classes, Interfaces and Enumerations

- The Concert class represents a concert event, with properties such as ID, artist, venue, date and time, and a list
of seats.
- The Seat class represents a seat in a concert, with properties like ID, seat number, seat type, price, and status.
It provides methods to book and release a seat.
- The SeatType enum represents the different types of seats available, such as regular, premium, and VIP.
- The SeatStatus enum represents the status of a seat, which can be available, booked, or reserved.
- The Booking class represents a booking made by a user for a specific concert and seats. It contains properties such
as ID, user, concert, seats, total price, and status. It provides methods to confirm and cancel a booking.
- The BookingStatus enum represents the status of a booking, which can be pending, confirmed, or cancelled.
- The User class represents a user of the concert ticket booking system, with properties like ID, name, and email.
- The ConcertTicketBookingSystem class is the central component of the system. It follows the Singleton pattern to ensure
a single instance of the system. It manages concerts, bookings, and provides methods to add concerts, search concerts,
book tickets, and cancel bookings.
- The SeatNotAvailableException is a custom exception used to handle cases where a seat is not available for booking.

*/

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use chrono::NaiveDateTime;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum SeatType {
    Regular,
    Premium,
    Vip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum SeatStatus {
    Available,
    Booked,
    Reserved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum BookingStatus {
    Pending,
    Confirmed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BookingError {
    #[error("concert not found")]
    ConcertNotFound,
    #[error("seat {seat_number} is not available: {reason}")]
    SeatNotAvailable { seat_number: String, reason: String },
    #[error("seat {seat_number} was selected more than once")]
    DuplicateSeat { seat_number: String },
    #[error("no seats were selected")]
    EmptySeatSelection,
    #[error("payment failed: {reason}")]
    PaymentFailed { reason: String },
    #[error("unknown booking")]
    UnknownBooking,
    #[error("booking cannot be cancelled in status {status}")]
    CancellationNotAllowed { status: BookingStatus },
}

pub type BookingResult<T> = std::result::Result<T, BookingError>;

#[derive(Debug, Clone)]
pub struct User {
    pub id: uuid::Uuid,
    pub name: String,
    pub email: String,
    pub phone: Option<String>,
}

impl User {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            email: email.into(),
            phone: None,
        }
    }

    pub fn with_phone(mut self, phone: impl Into<String>) -> Self {
        self.phone = Some(phone.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct Seat {
    pub number: String,
    pub seat_type: SeatType,
    pub price: f64,
    status: SeatStatus,
}

impl Seat {
    pub fn new(number: impl Into<String>, seat_type: SeatType, price: f64) -> Self {
        Self {
            number: number.into(),
            seat_type,
            price,
            status: SeatStatus::Available,
        }
    }

    pub fn status(&self) -> SeatStatus {
        self.status
    }

    fn reserve(&mut self) -> BookingResult<()> {
        if self.status != SeatStatus::Available {
            return Err(BookingError::SeatNotAvailable {
                seat_number: self.number.clone(),
                reason: format!("currently {}", self.status),
            });
        }
        self.status = SeatStatus::Reserved;
        Ok(())
    }

    fn confirm(&mut self) {
        self.status = SeatStatus::Booked;
    }

    // Idempotent on purpose: cancellation of a failed booking releases seats
    // that were never claimed.
    fn release(&mut self) {
        self.status = SeatStatus::Available;
    }
}

#[derive(Debug)]
pub struct Concert {
    pub id: uuid::Uuid,
    pub artist: String,
    pub venue: String,
    pub date_time: NaiveDateTime,
    seats: RwLock<Vec<Seat>>,
}

impl Concert {
    pub fn new(
        artist: impl Into<String>,
        venue: impl Into<String>,
        date_time: NaiveDateTime,
        seats: Vec<Seat>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            artist: artist.into(),
            venue: venue.into(),
            date_time,
            seats: RwLock::new(seats),
        }
    }

    pub fn seating(&self) -> Vec<Seat> {
        read_guard(&self.seats).clone()
    }

    pub fn available_seats(&self) -> usize {
        read_guard(&self.seats)
            .iter()
            .filter(|s| s.status() == SeatStatus::Available)
            .count()
    }
}

#[derive(Debug, Clone)]
pub struct Booking {
    pub id: uuid::Uuid,
    pub user: User,
    pub concert_id: uuid::Uuid,
    pub concert_artist: String,
    pub seats: Vec<Seat>,
    pub total_price: f64,
    pub status: BookingStatus,
    pub transaction_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Booking {
    pub fn confirm(&mut self) {
        self.status = BookingStatus::Confirmed;
    }

    pub fn cancel(&mut self) {
        self.status = BookingStatus::Cancelled;
    }
}

#[derive(Debug, Clone)]
pub struct WaitingListEntry {
    pub user: User,
    pub requested_seats: usize,
}

impl WaitingListEntry {
    fn new(user: User, requested_seats: usize) -> Self {
        Self {
            user,
            requested_seats,
        }
    }
}

pub trait PaymentGateway: Send + Sync {
    fn charge(&self, amount: f64, description: &str) -> BookingResult<String>;
}

#[derive(Default)]
pub struct MockPaymentGateway {
    pub always_fail: bool,
}

impl PaymentGateway for MockPaymentGateway {
    fn charge(&self, amount: f64, _description: &str) -> BookingResult<String> {
        if self.always_fail {
            return Err(BookingError::PaymentFailed {
                reason: "mock gateway declined the charge".into(),
            });
        }
        if amount <= 0.0 {
            return Err(BookingError::PaymentFailed {
                reason: format!("invalid amount {amount}"),
            });
        }
        Ok(uuid::Uuid::new_v4().to_string())
    }
}

pub trait NotificationService: Send + Sync {
    fn send_confirmation(&self, booking: &Booking);
    fn send_cancellation(&self, booking: &Booking);
}

#[derive(Default)]
pub struct LoggingNotificationService;

impl NotificationService for LoggingNotificationService {
    fn send_confirmation(&self, booking: &Booking) {
        let channel = if booking.user.phone.is_some() {
            "sms"
        } else {
            "email"
        };
        println!(
            "[notify:{channel}] {} <{}> — booking {} confirmed for {} ({} seats, ${:.2}, tx {})",
            booking.user.name,
            booking.user.email,
            booking.id,
            booking.concert_artist,
            booking.seats.len(),
            booking.total_price,
            booking.transaction_id
        );
    }

    fn send_cancellation(&self, booking: &Booking) {
        println!(
            "[notify:email] {} <{}> — booking {} cancelled, refund of ${:.2} processed",
            booking.user.name, booking.user.email, booking.id, booking.total_price
        );
    }
}

pub struct ConcertTicketBookingSystem {
    concerts: RwLock<HashMap<uuid::Uuid, Arc<Concert>>>,
    bookings: RwLock<HashMap<uuid::Uuid, Arc<RwLock<Booking>>>>,
    waiting_lists: RwLock<HashMap<uuid::Uuid, VecDeque<WaitingListEntry>>>,
    payment_gateway: Box<dyn PaymentGateway>,
    notifier: Box<dyn NotificationService>,
}

impl ConcertTicketBookingSystem {
    pub fn instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<ConcertTicketBookingSystem>> = OnceLock::new();
        INSTANCE.get_or_init(|| {
            Arc::new(ConcertTicketBookingSystem {
                concerts: RwLock::new(HashMap::new()),
                bookings: RwLock::new(HashMap::new()),
                waiting_lists: RwLock::new(HashMap::new()),
                payment_gateway: Box::new(MockPaymentGateway::default()),
                notifier: Box::new(LoggingNotificationService),
            })
        })
    }

    #[cfg(test)]
    fn with_services(
        payment_gateway: Box<dyn PaymentGateway>,
        notifier: Box<dyn NotificationService>,
    ) -> Self {
        Self {
            concerts: RwLock::new(HashMap::new()),
            bookings: RwLock::new(HashMap::new()),
            waiting_lists: RwLock::new(HashMap::new()),
            payment_gateway,
            notifier,
        }
    }

    pub fn add_concert(&self, concert: Concert) -> Arc<Concert> {
        let concert = Arc::new(concert);
        write_guard(&self.concerts).insert(concert.id, Arc::clone(&concert));
        concert
    }

    pub fn concerts(&self) -> Vec<Arc<Concert>> {
        read_guard(&self.concerts).values().cloned().collect()
    }

    pub fn search_concerts(
        &self,
        artist: Option<&str>,
        venue: Option<&str>,
        date: Option<NaiveDateTime>,
        time: Option<NaiveDateTime>,
    ) -> Vec<Arc<Concert>> {
        self.concerts()
            .into_iter()
            .filter(|c| artist.is_none_or(|a| c.artist.eq_ignore_ascii_case(a)))
            .filter(|c| venue.is_none_or(|v| c.venue.eq_ignore_ascii_case(v)))
            .filter(|c| date.is_none_or(|d| c.date_time.date() == d.date()))
            .filter(|c| time.is_none_or(|t| c.date_time.time() == t.time()))
            .collect()
    }

    fn concert(&self, concert_id: uuid::Uuid) -> BookingResult<Arc<Concert>> {
        read_guard(&self.concerts)
            .get(&concert_id)
            .cloned()
            .ok_or(BookingError::ConcertNotFound)
    }

    pub fn book_tickets(
        &self,
        user: &User,
        concert_id: uuid::Uuid,
        seat_numbers: &[String],
    ) -> BookingResult<Booking> {
        if seat_numbers.is_empty() {
            return Err(BookingError::EmptySeatSelection);
        }
        let mut seen = HashSet::new();
        for number in seat_numbers {
            if !seen.insert(number.clone()) {
                return Err(BookingError::DuplicateSeat {
                    seat_number: number.clone(),
                });
            }
        }

        let concert = self.concert(concert_id)?;
        let mut seats = write_guard(&concert.seats);

        let mut claimed: Vec<usize> = Vec::new();
        for number in seat_numbers {
            let claim = seats
                .iter()
                .position(|s| &s.number == number)
                .map(|idx| seats[idx].reserve().map(|()| idx));
            match claim {
                Some(Ok(idx)) => claimed.push(idx),
                Some(Err(e)) => {
                    for idx in &claimed {
                        seats[*idx].release();
                    }
                    return Err(e);
                }
                None => {
                    for idx in &claimed {
                        seats[*idx].release();
                    }
                    return Err(BookingError::SeatNotAvailable {
                        seat_number: number.clone(),
                        reason: "unknown seat in this concert".into(),
                    });
                }
            }
        }

        let total: f64 = claimed.iter().map(|&i| seats[i].price).sum();
        let transaction_id = match self.payment_gateway.charge(
            total,
            &format!("Concert: {} @ {}", concert.artist, concert.venue),
        ) {
            Ok(tx) => tx,
            Err(e) => {
                for idx in &claimed {
                    seats[*idx].release();
                }
                return Err(e);
            }
        };

        for idx in &claimed {
            seats[*idx].confirm();
        }
        let booked_seats: Vec<Seat> = claimed.iter().map(|&i| seats[i].clone()).collect();
        drop(seats);

        let mut booking = Booking {
            id: uuid::Uuid::new_v4(),
            user: user.clone(),
            concert_id,
            concert_artist: concert.artist.clone(),
            seats: booked_seats,
            total_price: total,
            status: BookingStatus::Pending,
            transaction_id,
            created_at: chrono::Utc::now(),
        };
        booking.confirm();

        let booking_id = booking.id;
        write_guard(&self.bookings).insert(booking_id, Arc::new(RwLock::new(booking)));
        let booking = self.booking(booking_id)?;
        self.notifier.send_confirmation(&booking);
        Ok(booking)
    }

    pub fn booking(&self, booking_id: uuid::Uuid) -> BookingResult<Booking> {
        read_guard(&self.bookings)
            .get(&booking_id)
            .map(|b| read_guard(b).clone())
            .ok_or(BookingError::UnknownBooking)
    }

    pub fn cancel_booking(&self, booking_id: uuid::Uuid) -> BookingResult<()> {
        let entry = read_guard(&self.bookings)
            .get(&booking_id)
            .cloned()
            .ok_or(BookingError::UnknownBooking)?;

        let mut booking = write_guard(&entry);
        match booking.status {
            BookingStatus::Confirmed => booking.cancel(),
            other => return Err(BookingError::CancellationNotAllowed { status: other }),
        }

        let concert = self.concert(booking.concert_id)?;
        let freed: Vec<String>;
        {
            let mut seats = write_guard(&concert.seats);
            for seat in seats.iter_mut() {
                if booking.seats.iter().any(|s| s.number == seat.number) {
                    seat.release();
                }
            }
            freed = booking.seats.iter().map(|s| s.number.clone()).collect();
        }

        self.notifier.send_cancellation(&booking);
        drop(booking);
        self.serve_waiting_list(concert.id, &freed);
        Ok(())
    }

    /// Offer the freed seats to the first waiting-list user who can take
    /// them. If their payment fails they are re-queued at the front.
    fn serve_waiting_list(&self, concert_id: uuid::Uuid, freed: &[String]) {
        let mut queue = write_guard(&self.waiting_lists);
        let Some(waiting) = queue.get_mut(&concert_id) else {
            return;
        };
        let Some(entry) = waiting.pop_front() else {
            return;
        };
        if entry.requested_seats > freed.len() {
            // The front user needs more seats than were freed; leave them
            // queued and stop — no one behind them can be served either.
            waiting.push_front(entry);
            return;
        }
        let seat_numbers: Vec<String> = freed[..entry.requested_seats].to_vec();
        match self.book_tickets(&entry.user, concert_id, &seat_numbers) {
            Ok(booking) => println!(
                "[system] waiting-list user {} booked {} seats for {}",
                entry.user.name,
                seat_numbers.len(),
                booking.concert_artist
            ),
            Err(_) => waiting.push_front(entry),
        }
    }

    pub fn join_waiting_list(
        &self,
        user: &User,
        concert_id: uuid::Uuid,
        seats: usize,
    ) -> BookingResult<()> {
        if seats == 0 {
            return Err(BookingError::EmptySeatSelection);
        }
        self.concert(concert_id)?;
        write_guard(&self.waiting_lists)
            .entry(concert_id)
            .or_default()
            .push_back(WaitingListEntry::new(user.clone(), seats));
        Ok(())
    }

    pub fn waiting_list_len(&self, concert_id: uuid::Uuid) -> usize {
        read_guard(&self.waiting_lists)
            .get(&concert_id)
            .map(|q| q.len())
            .unwrap_or(0)
    }
}

fn read_guard<T>(lock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    // A poisoned lock still holds valid data; the panic happened in an
    // earlier thread, not because the data is corrupt.
    lock.read().unwrap_or_else(|e| e.into_inner())
}

fn write_guard<T>(lock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    lock.write().unwrap_or_else(|e| e.into_inner())
}

fn run_demo() {
    use chrono::TimeZone;

    let system = ConcertTicketBookingSystem::instance();
    let show_time = chrono::Local
        .with_ymd_and_hms(2025, 12, 24, 19, 30, 0)
        .unwrap()
        .naive_local();

    let concert = system.add_concert(Concert::new(
        "Coldplay",
        "Madison Square Garden",
        show_time,
        vec![
            Seat::new("A1", SeatType::Vip, 250.0),
            Seat::new("A2", SeatType::Vip, 250.0),
            Seat::new("B1", SeatType::Premium, 150.0),
            Seat::new("B2", SeatType::Premium, 150.0),
            Seat::new("C1", SeatType::Regular, 75.0),
            Seat::new("C2", SeatType::Regular, 75.0),
        ],
    ));

    println!("== Seating ==");
    for seat in concert.seating() {
        println!(
            "  {} {:8} {:10} ${:.2}",
            seat.number,
            seat.seat_type,
            seat.status(),
            seat.price
        );
    }

    println!("\n== Search ==");
    let found = system.search_concerts(Some("coldplay"), Some("madison square garden"), None, None);
    for c in found {
        println!("  {} @ {} — {}", c.artist, c.venue, c.date_time);
    }

    println!("\n== Booking ==");
    let alice = User::new("Alice", "alice@example.com");
    let booking = system
        .book_tickets(&alice, concert.id, &["A1".to_string(), "B1".to_string()])
        .expect("booking succeeds");
    println!("  booking {} total ${:.2}", booking.id, booking.total_price);
    for seat in concert.seating() {
        println!(
            "  {} {:8} {:10}",
            seat.number,
            seat.seat_type,
            seat.status()
        );
    }

    let bob = User::new("Bob", "bob@example.com");
    match system.book_tickets(&bob, concert.id, &["A1".to_string()]) {
        Err(BookingError::SeatNotAvailable {
            seat_number,
            reason,
        }) => println!(
            "  Bob rejected: seat {seat_number} not available ({reason}) — no double-booking"
        ),
        other => panic!("expected SeatNotAvailable, got {other:?}"),
    }

    system
        .book_tickets(
            &bob,
            concert.id,
            &[
                "A2".to_string(),
                "B2".to_string(),
                "C1".to_string(),
                "C2".to_string(),
            ],
        )
        .expect("bob books the rest");
    system
        .join_waiting_list(
            &User::new("Carol", "carol@example.com").with_phone("+1-555-0100".to_string()),
            concert.id,
            1,
        )
        .expect("carol joins waiting list");
    println!(
        "\n  Waiting list size: {}",
        system.waiting_list_len(concert.id)
    );

    system
        .cancel_booking(booking.id)
        .expect("cancellation succeeds");
    println!(
        "  Waiting list size after cancellation: {}",
        system.waiting_list_len(concert.id)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn concert_with(n: usize) -> Concert {
        Concert::new(
            "Test Band",
            "Test Venue",
            chrono::Local::now().naive_local(),
            (0..n)
                .map(|i| Seat::new(format!("S{i}"), SeatType::Regular, 50.0))
                .collect(),
        )
    }

    fn system() -> ConcertTicketBookingSystem {
        ConcertTicketBookingSystem::with_services(
            Box::new(MockPaymentGateway::default()),
            Box::new(LoggingNotificationService),
        )
    }

    #[test]
    fn test_search_artist() {
        let sys = system();
        let t = chrono::Local::now().naive_local();
        sys.add_concert(Concert::new("Artist A", "Venue X", t, vec![]));
        sys.add_concert(Concert::new("Artist B", "Venue X", t, vec![]));
        assert_eq!(
            sys.search_concerts(Some("artist a"), None, None, None)
                .len(),
            1
        );
    }

    #[test]
    fn test_search_venue() {
        let sys = system();
        let t = chrono::Local::now().naive_local();
        sys.add_concert(Concert::new("Artist A", "Venue X", t, vec![]));
        sys.add_concert(Concert::new("Artist B", "Venue Y", t, vec![]));
        assert_eq!(
            sys.search_concerts(None, Some("venue x"), None, None).len(),
            1
        );
    }

    #[test]
    fn test_search_date() {
        let sys = system();
        let today = chrono::Local::now().naive_local().date();
        let tomorrow = today.succ_opt().unwrap();
        sys.add_concert(Concert::new(
            "Artist A",
            "Venue X",
            today.and_hms_opt(20, 0, 0).unwrap(),
            vec![],
        ));
        sys.add_concert(Concert::new(
            "Artist B",
            "Venue X",
            tomorrow.and_hms_opt(20, 0, 0).unwrap(),
            vec![],
        ));
        assert_eq!(
            sys.search_concerts(None, None, Some(today.and_hms_opt(0, 0, 0).unwrap()), None)
                .len(),
            1
        );
    }

    #[test]
    fn test_search_time() {
        let sys = system();
        let today = chrono::Local::now().naive_local().date();
        let evening = today.and_hms_opt(20, 0, 0).unwrap();
        let afternoon = today.and_hms_opt(14, 0, 0).unwrap();
        sys.add_concert(Concert::new("Artist A", "Venue X", evening, vec![]));
        sys.add_concert(Concert::new("Artist B", "Venue X", afternoon, vec![]));
        assert_eq!(
            sys.search_concerts(None, None, None, Some(evening)).len(),
            1
        );
    }

    #[test]
    fn test_search_miss() {
        let sys = system();
        let t = chrono::Local::now().naive_local();
        sys.add_concert(Concert::new("Artist A", "Venue X", t, vec![]));
        assert_eq!(sys.search_concerts(Some("nope"), None, None, None).len(), 0);
    }

    #[test]
    fn test_no_double_book() {
        let sys = system();
        let concert = sys.add_concert(concert_with(2));
        let alice = User::new("Alice", "a@x.io");
        sys.book_tickets(&alice, concert.id, &["S0".to_string()])
            .unwrap();
        let err = sys
            .book_tickets(&User::new("Bob", "b@x.io"), concert.id, &["S0".to_string()])
            .unwrap_err();
        assert!(matches!(err, BookingError::SeatNotAvailable { .. }));
    }

    #[test]
    fn test_concurrent_single_winner() {
        let sys = Arc::new(system());
        let concert = sys.add_concert(concert_with(8));
        let handles: Vec<_> = (0..8)
            .map(|i| {
                let sys = Arc::clone(&sys);
                let id = concert.id;
                std::thread::spawn(move || {
                    sys.book_tickets(
                        &User::new(format!("user{i}"), format!("u{i}@x.io")),
                        id,
                        &["S0".to_string()],
                    )
                })
            })
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|r| matches!(r, Err(BookingError::SeatNotAvailable { .. })))
                .count(),
            7
        );
    }

    #[test]
    fn test_payment_failure_rollback() {
        let sys = ConcertTicketBookingSystem::with_services(
            Box::new(MockPaymentGateway { always_fail: true }),
            Box::new(LoggingNotificationService),
        );
        let concert = sys.add_concert(concert_with(2));
        let err = sys
            .book_tickets(
                &User::new("Alice", "a@x.io"),
                concert.id,
                &["S0".to_string()],
            )
            .unwrap_err();
        assert!(matches!(err, BookingError::PaymentFailed { .. }));
        assert_eq!(concert.available_seats(), 2);
    }

    #[test]
    fn test_cancel_feeds_waitlist() {
        let sys = system();
        let concert = sys.add_concert(concert_with(1));
        let alice = User::new("Alice", "a@x.io");
        let booking = sys
            .book_tickets(&alice, concert.id, &["S0".to_string()])
            .unwrap();
        sys.join_waiting_list(&User::new("Carol", "c@x.io"), concert.id, 1)
            .unwrap();
        sys.cancel_booking(booking.id).unwrap();
        assert_eq!(sys.waiting_list_len(concert.id), 0);
        assert_eq!(concert.available_seats(), 0);
    }

    #[test]
    fn test_waitlist_oversized() {
        let sys = system();
        let concert = sys.add_concert(concert_with(2));
        let alice = User::new("Alice", "a@x.io");
        let booking = sys
            .book_tickets(&alice, concert.id, &["S0".to_string()])
            .unwrap();
        sys.join_waiting_list(&User::new("Carol", "c@x.io"), concert.id, 2)
            .unwrap();
        sys.cancel_booking(booking.id).unwrap();
        assert_eq!(sys.waiting_list_len(concert.id), 1);
        assert_eq!(concert.available_seats(), 2);
    }

    #[test]
    fn test_duplicate_seats_rejected() {
        let sys = system();
        let concert = sys.add_concert(concert_with(2));
        let err = sys
            .book_tickets(
                &User::new("Alice", "a@x.io"),
                concert.id,
                &["S0".to_string(), "S0".to_string()],
            )
            .unwrap_err();
        assert!(matches!(err, BookingError::DuplicateSeat { .. }));
        assert_eq!(concert.available_seats(), 2);
    }
}

fn main() {
    run_demo();
    println!("\nAll done.");
}
