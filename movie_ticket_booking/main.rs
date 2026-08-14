//! Movie ticket booking system. Full requirements and design narrative:
//! see `readme.md`. This is the flight_management-style architecture kept
//! in a single file: domain (seats/shows/bookings/payments) + adapters
//! (payment gateway) + a demo driver, all here for easy reading.
//!
//! Seat lifecycle: a booking reserves seats (SeatStatus::Reserved), pays
//! through the gateway *outside* the seat lock, then confirms (Booked) or
//! rolls back (release) on payment failure. The reservation under a write
//! lock is what makes double-booking impossible.

//! # Designing a Movie Ticket Booking System like BookMyShow
//! ### Requirements
//!
//! - The system should allow users to view the list of movies playing in different theaters.
//! - Users should be able to select a movie, theater, and show timing to book tickets.
//! - The system should display the seating arrangement of the selected show and allow users to choose seats.
//! - Users should be able to make payments and confirm their booking.
//! - The system should handle concurrent bookings and ensure seat availability is updated in real-time.
//! - The system should support different types of seats (e.g., normal, premium) and pricing.
//! - The system should allow theater administrators to add, update, and remove movies, shows, and seating
//! arrangements.
//! - The system should be scalable to handle a large number of concurrent users and bookings.
//!
//! ### Classes, Interfaces and Enumerations
//!
//! - The Movie class represents a movie with properties such as ID, title, description, and duration.
//! - The Theater class represents a theater with properties such as ID, name, location, and a list of shows.
//! - The Show class represents a movie show in a theater, with properties such as ID, movie, theater, start time,
//! end time, and a map of seats.
//! - The Seat class represents a seat in a show, with properties such as ID, row, column, type, price, and status.
//! - The SeatType enum defines the different types of seats (normal or premium).
//! - The SeatStatus enum defines the different statuses of a seat (available or booked).
//! - The Booking class represents a booking made by a user, with properties such as ID, user, show, selected seats,
//! total price, and status.
//! - The BookingStatus enum defines the different statuses of a booking (pending, confirmed, or cancelled).
//! - The User class represents a user of the booking system, with properties such as ID, name, and email.
//! - The MovieTicketBookingSystem class is the main class that manages the movie ticket booking system. It follows
//! the Singleton pattern to ensure only one instance of the system exists.
//! - The MovieTicketBookingSystem class provides methods for adding movies, theaters, and shows, as well as booking
//! tickets, confirming bookings, and cancelling bookings.
//! - Multi-threading is achieved using concurrent data structures such as ConcurrentHashMap to handle concurrent access
//! to shared resources like shows and bookings.
//! - The MovieTicketBookingDemo class demonstrates the usage of the movie ticket booking system by adding movies, theaters,
//! shows, booking tickets, and confirming or cancelling bookings.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum SeatType {
    Normal,
    Premium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum SeatStatus {
    /// Seat is held while payment is being processed.
    Available,
    Reserved,
    Booked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum BookingStatus {
    Confirmed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentMethod {
    Card,
    Upi,
    Wallet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum_macros::Display)]
pub enum PaymentStatus {
    Completed,
    PartiallyRefunded,
    Refunded,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BookingError {
    #[error("movie '{title}' already exists")]
    MovieExists { title: String },
    #[error("movie '{title}' not found")]
    MovieNotFound { title: String },
    #[error("theater '{name}' not found")]
    TheaterNotFound { name: String },
    #[error("show not found")]
    ShowNotFound,
    #[error("seat {seat} not found in this show")]
    SeatNotFound { seat: String },
    #[error("seat {seat} is not available: {reason}")]
    SeatNotAvailable { seat: String, reason: String },
    #[error("seat {seat} was selected more than once")]
    DuplicateSeat { seat: String },
    #[error("no seats were selected")]
    EmptySeatSelection,
    #[error("booking not found")]
    UnknownBooking,
    #[error("booking cannot be cancelled in status {status}")]
    CancellationNotAllowed { status: BookingStatus },
    #[error("permission denied")]
    PermissionDenied,
    #[error("payment failed: {0}")]
    PaymentFailed(String),
}

pub type BookingResult<T> = std::result::Result<T, BookingError>;

#[derive(Debug, Clone)]
pub struct Movie {
    pub id: uuid::Uuid,
    pub title: String,
    pub description: String,
    pub duration_in_mins: u32,
}

impl Movie {
    pub fn new(
        title: impl Into<String>,
        description: impl Into<String>,
        duration_in_mins: u32,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            title: title.into(),
            description: description.into(),
            duration_in_mins,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Theater {
    pub id: uuid::Uuid,
    pub name: String,
    pub location: String,
}

#[derive(Debug, Clone)]
pub struct Seat {
    pub id: uuid::Uuid,
    pub row: String,
    pub column: u32,
    pub seat_type: SeatType,
    pub price: f64,
    status: SeatStatus,
    /// Customer id holding this seat (Reserved or Booked).
    holder: Option<uuid::Uuid>,
}

impl Seat {
    pub fn new(row: impl Into<String>, column: u32, seat_type: SeatType, price: f64) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            row: row.into(),
            column,
            seat_type,
            price,
            status: SeatStatus::Available,
            holder: None,
        }
    }

    pub fn number(&self) -> String {
        format!("{}{}", self.row, self.column)
    }

    pub fn status(&self) -> SeatStatus {
        self.status
    }

    pub fn holder(&self) -> Option<uuid::Uuid> {
        self.holder
    }
}

/// The show inventory. Interior mutability is required because a Show is
/// shared (`Arc`) across the system and concurrent bookings.
#[derive(Debug)]
pub struct Show {
    pub id: uuid::Uuid,
    pub movie_title: String,
    pub theater_name: String,
    pub start_time: chrono::NaiveDateTime,
    pub end_time: chrono::NaiveDateTime,
    seats: RwLock<HashMap<String, Seat>>,
}

impl Show {
    pub fn new(
        movie_title: impl Into<String>,
        theater_name: impl Into<String>,
        start_time: chrono::NaiveDateTime,
        seats: Vec<Seat>,
    ) -> Self {
        let seats = seats
            .into_iter()
            .map(|seat| (seat.number(), seat))
            .collect();
        Self {
            id: uuid::Uuid::new_v4(),
            movie_title: movie_title.into(),
            theater_name: theater_name.into(),
            start_time,
            end_time: start_time + chrono::Duration::minutes(150),
            seats: RwLock::new(seats),
        }
    }

    /// Hold a seat for a customer while payment is processed. Returns the
    /// seat type and price; the caller owns the seat lifecycle and must
    /// call `confirm` or `release` when the payment outcome is known.
    pub(crate) fn reserve(
        &self,
        seat_number: &str,
        holder: uuid::Uuid,
    ) -> BookingResult<(SeatType, f64)> {
        let mut seats = write_guard(&self.seats);
        let seat = seats
            .get_mut(seat_number)
            .ok_or_else(|| BookingError::SeatNotFound {
                seat: seat_number.to_string(),
            })?;
        if seat.status != SeatStatus::Available {
            return Err(BookingError::SeatNotAvailable {
                seat: seat_number.to_string(),
                reason: format!("currently {}", seat.status),
            });
        }
        seat.status = SeatStatus::Reserved;
        seat.holder = Some(holder);
        Ok((seat.seat_type, seat.price))
    }

    /// Mark a reserved seat as paid for.
    pub(crate) fn confirm(&self, seat_number: &str) {
        if let Some(seat) = write_guard(&self.seats).get_mut(seat_number) {
            seat.status = SeatStatus::Booked;
        }
    }

    /// Return a reserved or booked seat to the pool. Idempotent on purpose:
    /// a failed payment may release seats that were never reserved.
    pub(crate) fn release(&self, seat_number: &str) {
        if let Some(seat) = write_guard(&self.seats).get_mut(seat_number) {
            seat.status = SeatStatus::Available;
            seat.holder = None;
        }
    }

    /// Read-only snapshot of the seating, sorted by row then column.
    pub fn seating(&self) -> Vec<Seat> {
        let mut seats: Vec<Seat> = read_guard(&self.seats).values().cloned().collect();
        seats.sort_by_key(|a| a.number());
        seats
    }

    pub fn available_seats(&self) -> usize {
        read_guard(&self.seats)
            .values()
            .filter(|seat| seat.status == SeatStatus::Available)
            .count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRole {
    Customer,
    Admin,
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: uuid::Uuid,
    pub name: String,
    pub email: String,
    pub role: UserRole,
}

impl User {
    pub fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            email: email.into(),
            role: UserRole::Customer,
        }
    }

    pub fn admin(name: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4(),
            name: name.into(),
            email: email.into(),
            role: UserRole::Admin,
        }
    }

    pub fn is_admin(&self) -> bool {
        self.role == UserRole::Admin
    }
}

#[derive(Debug, Clone)]
pub struct Booking {
    pub id: uuid::Uuid,
    pub user: User,
    pub show_id: uuid::Uuid,
    pub movie_title: String,
    pub seat_numbers: Vec<String>,
    pub total_price: f64,
    pub refunded: f64,
    pub status: BookingStatus,
}

#[derive(Debug, Clone)]
pub struct Payment {
    pub payment_id: u64,
    pub booking_id: uuid::Uuid,
    pub amount: f64,
    pub refunded: f64,
    pub method: PaymentMethod,
    pub txn_id: Option<String>,
    pub status: PaymentStatus,
}

#[derive(Debug, Clone)]
pub struct CancelResult {
    pub booking_id: uuid::Uuid,
    pub refunded: f64,
}

/// Extension seam: swap the implementation for a real PSP without touching
/// the domain. Charge/refund errors are provider strings, mapped to
/// `BookingError::PaymentFailed` at the boundary.
pub trait PaymentGateway: Send + Sync {
    fn charge(&self, amount: f64, method: PaymentMethod) -> Result<String, String>;
    fn refund(&self, txn_id: &str, amount: f64) -> Result<(), String>;
}

/// Simulates an external PSP. Set `fail_next` to exercise the rollback path.
#[derive(Default)]
pub struct MockGateway {
    next_txn: AtomicU64,
    fail_next: AtomicBool,
}

impl MockGateway {
    pub fn fail_next(&self) {
        self.fail_next.store(true, Ordering::SeqCst);
    }
}

impl PaymentGateway for Arc<MockGateway> {
    #[allow(unused)]
    fn charge(&self, amount: f64, method: PaymentMethod) -> std::result::Result<String, String> {
        if self.fail_next.swap(false, Ordering::SeqCst) {
            return Err("mock gateway rejected charge".into());
        }
        Ok(format!(
            "TXN-{:06}",
            self.next_txn.fetch_add(1, Ordering::Relaxed) + 1
        ))
    }

    #[allow(unused)]
    fn refund(&self, txn_id: &str, amount: f64) -> std::result::Result<(), String> {
        Ok(())
    }
}

/// Records completed and failed charges; failed ones are the audit trail.
pub struct PaymentProcessor {
    next_payment: AtomicU64,
    payments: RwLock<HashMap<u64, Payment>>,
    gateway: Box<dyn PaymentGateway>,
}

impl PaymentProcessor {
    pub fn new(gateway: Box<dyn PaymentGateway>) -> Self {
        Self {
            next_payment: AtomicU64::new(0),
            payments: RwLock::new(HashMap::new()),
            gateway,
        }
    }

    pub fn process(
        &self,
        booking_id: uuid::Uuid,
        amount: f64,
        method: PaymentMethod,
    ) -> BookingResult<Payment> {
        let payment_id = self.next_payment.fetch_add(1, Ordering::Relaxed) + 1;
        let (txn_id, status) = match self.gateway.charge(amount, method) {
            Ok(txn) => (Some(txn), PaymentStatus::Completed),
            Err(_) => (None, PaymentStatus::Failed),
        };
        let payment = Payment {
            payment_id,
            booking_id,
            amount,
            refunded: 0.0,
            method,
            txn_id,
            status,
        };
        write_guard(&self.payments).insert(payment_id, payment.clone());
        if payment.status == PaymentStatus::Failed {
            Err(BookingError::PaymentFailed(format!(
                "charge for booking {booking_id} failed"
            )))
        } else {
            Ok(payment)
        }
    }

    /// Refund the charge attached to a booking. Returns the amount actually
    /// applied; supports partial refunds.
    pub fn refund(&self, booking_id: uuid::Uuid, amount: f64) -> BookingResult<f64> {
        let candidates: Vec<Payment> = read_guard(&self.payments)
            .values()
            .filter(|payment| payment.booking_id == booking_id)
            .cloned()
            .collect();
        let mut remaining = amount;
        let mut applied = 0.0;
        for mut payment in candidates {
            if remaining <= f64::EPSILON {
                break;
            }
            if payment.status != PaymentStatus::Completed {
                continue;
            }
            let take = remaining.min(payment.amount - payment.refunded);
            if take <= f64::EPSILON {
                continue;
            }
            let txn_id = payment
                .txn_id
                .as_deref()
                .ok_or(BookingError::PaymentFailed(
                    "payment has no gateway transaction".into(),
                ))?;
            self.gateway
                .refund(txn_id, take)
                .map_err(BookingError::PaymentFailed)?;
            payment.refunded += take;
            payment.status = if (payment.amount - payment.refunded).abs() < f64::EPSILON {
                PaymentStatus::Refunded
            } else {
                PaymentStatus::PartiallyRefunded
            };
            write_guard(&self.payments).insert(payment.payment_id, payment);
            remaining -= take;
            applied += take;
        }
        Ok(applied)
    }

    pub fn payment(&self, payment_id: u64) -> Option<Payment> {
        read_guard(&self.payments).get(&payment_id).cloned()
    }
}

/// Composition root. Shared components are built here and handed out via
/// `Arc` — no process-global state, so tests can build independent systems.
pub struct MovieTicketBookingSystem {
    movies: RwLock<HashMap<String, Movie>>,
    theaters: RwLock<HashMap<String, Theater>>,
    shows: RwLock<HashMap<uuid::Uuid, Arc<Show>>>,
    bookings: RwLock<HashMap<uuid::Uuid, Arc<RwLock<Booking>>>>,
    pub payments: Arc<PaymentProcessor>,
}

impl MovieTicketBookingSystem {
    pub fn with_gateway(gateway: Box<dyn PaymentGateway>) -> Self {
        Self {
            movies: RwLock::new(HashMap::new()),
            theaters: RwLock::new(HashMap::new()),
            shows: RwLock::new(HashMap::new()),
            bookings: RwLock::new(HashMap::new()),
            payments: Arc::new(PaymentProcessor::new(gateway)),
        }
    }

    pub fn new() -> Self {
        Self::with_gateway(Box::new(Arc::new(MockGateway::default())))
    }

    /// The process-wide default instance, for demo convenience only.
    pub fn instance() -> &'static Arc<Self> {
        static INSTANCE: OnceLock<Arc<MovieTicketBookingSystem>> = OnceLock::new();
        INSTANCE.get_or_init(|| Arc::new(MovieTicketBookingSystem::new()))
    }

    fn ensure_admin(&self, actor: &User) -> BookingResult<()> {
        if actor.is_admin() {
            Ok(())
        } else {
            Err(BookingError::PermissionDenied)
        }
    }

    fn find_show(&self, show_id: uuid::Uuid) -> BookingResult<Arc<Show>> {
        read_guard(&self.shows)
            .get(&show_id)
            .cloned()
            .ok_or(BookingError::ShowNotFound)
    }

    pub fn add_movie(&self, actor: &User, movie: Movie) -> BookingResult<()> {
        self.ensure_admin(actor)?;
        let mut movies = write_guard(&self.movies);
        if movies.contains_key(&movie.title) {
            return Err(BookingError::MovieExists {
                title: movie.title.clone(),
            });
        }
        movies.insert(movie.title.clone(), movie);
        Ok(())
    }

    pub fn add_theater(&self, actor: &User, name: &str, location: &str) -> BookingResult<()> {
        self.ensure_admin(actor)?;
        let mut theaters = write_guard(&self.theaters);
        if theaters.contains_key(name) {
            return Err(BookingError::TheaterNotFound {
                name: name.to_string(),
            });
        }
        theaters.insert(
            name.to_string(),
            Theater {
                id: uuid::Uuid::new_v4(),
                name: name.to_string(),
                location: location.to_string(),
            },
        );
        Ok(())
    }

    pub fn add_show(
        &self,
        actor: &User,
        movie_title: &str,
        theater_name: &str,
        start_time: chrono::NaiveDateTime,
        seats: Vec<Seat>,
    ) -> BookingResult<Arc<Show>> {
        self.ensure_admin(actor)?;
        if !read_guard(&self.movies).contains_key(movie_title) {
            return Err(BookingError::MovieNotFound {
                title: movie_title.to_string(),
            });
        }
        if !read_guard(&self.theaters).contains_key(theater_name) {
            return Err(BookingError::TheaterNotFound {
                name: theater_name.to_string(),
            });
        }
        let show = Arc::new(Show::new(movie_title, theater_name, start_time, seats));
        write_guard(&self.shows).insert(show.id, Arc::clone(&show));
        Ok(show)
    }

    pub fn movies(&self) -> Vec<Movie> {
        read_guard(&self.movies).values().cloned().collect()
    }

    pub fn search_movies(&self, query: &str) -> Vec<Movie> {
        read_guard(&self.movies)
            .values()
            .filter(|movie| movie.title.to_lowercase().contains(&query.to_lowercase()))
            .cloned()
            .collect()
    }

    pub fn shows_for_movie(&self, movie_title: &str) -> Vec<Arc<Show>> {
        read_guard(&self.shows)
            .values()
            .filter(|show| show.movie_title == movie_title)
            .cloned()
            .collect()
    }

    /// Book seats and pay as one transaction:
    ///   1. RESERVE every selected seat under the show's write lock.
    ///   2. PAY through the gateway outside the lock — gateway latency must
    ///      not block other customers.
    ///   3. CONFIRM (booked) or ROLL BACK (release) on payment failure.
    ///
    /// Never hold two locks at once; the reservation under a write lock is
    /// what makes double-booking impossible.
    pub fn book_tickets(
        &self,
        user: &User,
        show: &Show,
        seat_numbers: &[String],
        method: PaymentMethod,
    ) -> BookingResult<(Booking, Payment)> {
        if seat_numbers.is_empty() {
            return Err(BookingError::EmptySeatSelection);
        }
        let mut seen = HashMap::new();
        for number in seat_numbers {
            if seen.insert(number.clone(), ()).is_some() {
                return Err(BookingError::DuplicateSeat {
                    seat: number.clone(),
                });
            }
        }

        // Reserve all seats up front; on any conflict release the ones
        // already claimed so the request is atomic.
        let mut total = 0.0;
        let mut claimed: Vec<String> = Vec::new();
        for number in seat_numbers {
            match show.reserve(number, user.id) {
                Ok((_, price)) => {
                    total += price;
                    claimed.push(number.clone());
                }
                Err(e) => {
                    for claimed_number in &claimed {
                        show.release(claimed_number);
                    }
                    return Err(e);
                }
            }
        }

        let booking = Booking {
            id: uuid::Uuid::new_v4(),
            user: user.clone(),
            show_id: show.id,
            movie_title: show.movie_title.clone(),
            seat_numbers: claimed.clone(),
            total_price: total,
            refunded: 0.0,
            status: BookingStatus::Confirmed,
        };
        let booking_id = booking.id;

        // Pay outside the seat lock; roll back the reservation on failure.
        let payment = match self.payments.process(booking_id, total, method) {
            Ok(payment) => payment,
            Err(err) => {
                for number in &claimed {
                    show.release(number);
                }
                return Err(err);
            }
        };
        for number in &claimed {
            show.confirm(number);
        }
        write_guard(&self.bookings).insert(booking_id, Arc::new(RwLock::new(booking)));
        Ok((self.booking(booking_id)?, payment))
    }

    pub fn booking(&self, booking_id: uuid::Uuid) -> BookingResult<Booking> {
        read_guard(&self.bookings)
            .get(&booking_id)
            .map(|b| read_guard(b).clone())
            .ok_or(BookingError::UnknownBooking)
    }

    /// Cancel a confirmed booking: mark Cancelled, release its seats, then
    /// refund. Booking state is committed before the refund so a refund
    /// failure can never leave a seat locked.
    pub fn cancel_booking(
        &self,
        booking_id: uuid::Uuid,
        acting_user: &User,
    ) -> BookingResult<CancelResult> {
        let (show_id, seat_numbers, remaining_refund) = {
            let bookings = write_guard(&self.bookings);
            let entry = bookings
                .get(&booking_id)
                .cloned()
                .ok_or(BookingError::UnknownBooking)?;
            let mut booking = write_guard(&entry);
            if !acting_user.is_admin() && booking.user.id != acting_user.id {
                return Err(BookingError::PermissionDenied);
            }
            if booking.status != BookingStatus::Confirmed {
                return Err(BookingError::CancellationNotAllowed {
                    status: booking.status,
                });
            }
            booking.status = BookingStatus::Cancelled;
            (
                booking.show_id,
                booking.seat_numbers.clone(),
                booking.total_price - booking.refunded,
            )
        };

        let show = self.find_show(show_id)?;
        for number in &seat_numbers {
            show.release(number);
        }
        let refunded = if remaining_refund > 0.0 {
            self.payments.refund(booking_id, remaining_refund)?
        } else {
            0.0
        };
        Ok(CancelResult {
            booking_id,
            refunded,
        })
    }
}

impl Default for MovieTicketBookingSystem {
    fn default() -> Self {
        Self::new()
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

/// Build a hall of `rows` rows × `cols` columns; the first `premium_rows`
/// are premium seats.
fn standard_hall(rows: u32, cols: u32, premium_rows: u32) -> Vec<Seat> {
    let row_letter = |r: u32| char::from(b'A' + r as u8);
    let mut seats = Vec::new();
    for r in 0..rows {
        let seat_type = if r < premium_rows {
            SeatType::Premium
        } else {
            SeatType::Normal
        };
        let price = if r < premium_rows { 500.0 } else { 250.0 };
        for c in 1..=cols {
            seats.push(Seat::new(row_letter(r), c, seat_type, price));
        }
    }
    seats
}

fn run_demo() {
    let system = MovieTicketBookingSystem::new();
    let admin = User::admin("Theater Ops", "ops@example.com");
    system
        .add_movie(
            &admin,
            Movie::new(
                "Interstellar",
                "A team of explorers travel through a wormhole",
                169,
            ),
        )
        .expect("movie added");
    system
        .add_theater(&admin, "PVR IMAX", "Mumbai")
        .expect("theater added");
    let show = system
        .add_show(
            &admin,
            "Interstellar",
            "PVR IMAX",
            chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
            standard_hall(4, 3, 1),
        )
        .expect("show added");

    println!("=== Movies matching 'inter' ===");
    for movie in system.search_movies("inter") {
        println!("  {} ({} min)", movie.title, movie.duration_in_mins);
    }

    println!("\n=== Seating for {} ===", show.movie_title);
    for seat in show.seating() {
        println!(
            "  {} {:8} {:10} ${:.2}",
            seat.number(),
            seat.seat_type,
            seat.status(),
            seat.price
        );
    }

    println!("\n=== Booking ===");
    let alice = User::new("Alice", "alice@example.com");
    let (booking, payment) = system
        .book_tickets(
            &alice,
            &show,
            &["A1".to_string(), "B1".to_string()],
            PaymentMethod::Card,
        )
        .expect("booking succeeds");
    println!(
        "  booking {} confirmed, total ${:.2}, payment {} [{}]",
        booking.id, booking.total_price, payment.payment_id, payment.status
    );

    let bob = User::new("Bob", "bob@example.com");
    match system.book_tickets(&bob, &show, &["A1".to_string()], PaymentMethod::Upi) {
        Err(BookingError::SeatNotAvailable { seat, reason }) => {
            println!("  Bob rejected: seat {seat} not available ({reason})")
        }
        other => panic!("expected SeatNotAvailable, got {other:?}"),
    }

    let cancelled = system
        .cancel_booking(booking.id, &alice)
        .expect("cancellation succeeds");
    println!("\n=== Cancelled {} by owner ===", cancelled.booking_id);
    println!("  refunded ${:.2}", cancelled.refunded);
    println!(
        "  A1 is now {} and {} seats are free",
        show.seating()
            .into_iter()
            .find(|seat| seat.number() == "A1")
            .map(|seat| seat.status().to_string())
            .unwrap_or_else(|| "?".into()),
        show.available_seats()
    );

    println!("\n=== Payment failure rolls back ===");
    let gateway = Arc::new(MockGateway::default());
    let failing = MovieTicketBookingSystem::with_gateway(Box::new(Arc::clone(&gateway)));
    failing
        .add_movie(&admin, Movie::new("Dune", "Desert planet", 155))
        .expect("movie added");
    failing
        .add_theater(&admin, "IMAX", "Delhi")
        .expect("theater added");
    let dune_show = failing
        .add_show(
            &admin,
            "Dune",
            "IMAX",
            chrono::Utc::now().naive_utc() + chrono::Duration::hours(2),
            standard_hall(2, 2, 1),
        )
        .expect("show added");
    gateway.fail_next();
    match failing.book_tickets(
        &alice,
        &dune_show,
        &["A1".to_string()],
        PaymentMethod::Wallet,
    ) {
        Err(BookingError::PaymentFailed(_)) => println!("  charge rejected as expected"),
        other => panic!("expected PaymentFailed, got {other:?}"),
    }
    println!(
        "  A1 returned to the pool: {}",
        dune_show
            .seating()
            .into_iter()
            .find(|seat| seat.number() == "A1")
            .map(|seat| seat.status().to_string())
            .unwrap_or_else(|| "?".into())
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn system() -> MovieTicketBookingSystem {
        MovieTicketBookingSystem::new()
    }

    fn admin() -> User {
        User::admin("Admin", "admin@example.com")
    }

    fn seeded_show() -> (MovieTicketBookingSystem, Arc<Show>) {
        let system = system();
        let admin = admin();
        system
            .add_movie(&admin, Movie::new("Interstellar", "space", 169))
            .expect("movie added");
        system
            .add_theater(&admin, "PVR", "Mumbai")
            .expect("theater added");
        let show = system
            .add_show(
                &admin,
                "Interstellar",
                "PVR",
                chrono::Utc::now().naive_utc(),
                vec![
                    Seat::new("A", 1, SeatType::Premium, 500.0),
                    Seat::new("A", 2, SeatType::Premium, 500.0),
                ],
            )
            .expect("show added");
        (system, show)
    }

    fn seeded_hall() -> (MovieTicketBookingSystem, Arc<Show>) {
        let system = system();
        let admin = admin();
        system
            .add_movie(&admin, Movie::new("Interstellar", "space", 169))
            .expect("movie added");
        system
            .add_theater(&admin, "PVR", "Mumbai")
            .expect("theater added");
        let show = system
            .add_show(
                &admin,
                "Interstellar",
                "PVR",
                chrono::Utc::now().naive_utc(),
                standard_hall(6, 6, 1),
            )
            .expect("show added");
        (system, show)
    }

    fn customer(name: &str) -> User {
        User::new(name, "customer@example.com")
    }

    #[test]
    fn test_duplicate_movie() {
        let system = system();
        let admin = admin();
        system
            .add_movie(&admin, Movie::new("Interstellar", "space", 169))
            .expect("first add");
        let result = system.add_movie(&admin, Movie::new("Interstellar", "space", 169));
        assert!(matches!(result, Err(BookingError::MovieExists { .. })));
    }

    #[test]
    fn test_admin_required() {
        let system = system();
        let result = system.add_movie(&customer("Alice"), Movie::new("Dune", "space", 155));
        assert!(matches!(result, Err(BookingError::PermissionDenied)));
    }

    #[test]
    fn test_show_missing_movie() {
        let system = system();
        let admin = admin();
        system
            .add_theater(&admin, "PVR", "Mumbai")
            .expect("theater added");
        let result = system.add_show(
            &admin,
            "Interstellar",
            "PVR",
            chrono::Utc::now().naive_utc(),
            vec![],
        );
        assert!(matches!(result, Err(BookingError::MovieNotFound { .. })));
    }

    #[test]
    fn test_book_missing_seat() {
        let (sys, show) = seeded_show();
        let result = sys.book_tickets(
            &customer("Alice"),
            &show,
            &["Z9".to_string()],
            PaymentMethod::Card,
        );
        assert!(matches!(result, Err(BookingError::SeatNotFound { .. })));
    }

    #[test]
    fn test_duplicate_seats_rejected() {
        let (sys, show) = seeded_show();
        let result = sys.book_tickets(
            &customer("Alice"),
            &show,
            &["A1".to_string(), "A1".to_string()],
            PaymentMethod::Card,
        );
        assert!(matches!(result, Err(BookingError::DuplicateSeat { .. })));
    }

    #[test]
    fn test_no_double_book() {
        let (sys, show) = seeded_show();
        let alice = customer("Alice");
        sys.book_tickets(&alice, &show, &["A1".to_string()], PaymentMethod::Card)
            .expect("first booking");
        assert_eq!(
            show.seating()
                .into_iter()
                .find(|seat| seat.number() == "A1")
                .unwrap()
                .holder(),
            Some(alice.id)
        );
        let result = sys.book_tickets(
            &customer("Bob"),
            &show,
            &["A1".to_string()],
            PaymentMethod::Card,
        );
        assert!(matches!(result, Err(BookingError::SeatNotAvailable { .. })));
    }

    #[test]
    fn test_seat_race() {
        let (sys, show) = seeded_show();
        let sys = Arc::new(sys);
        let handles: Vec<_> = (0..64)
            .map(|i| {
                let sys = Arc::clone(&sys);
                let show = Arc::clone(&show);
                std::thread::spawn(move || {
                    sys.book_tickets(
                        &customer(&format!("user{i}")),
                        &show,
                        &["A1".to_string()],
                        PaymentMethod::Card,
                    )
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
        let (sys, show) = seeded_hall();
        let sys = Arc::new(sys);
        let handles: Vec<_> = show
            .seating()
            .into_iter()
            .map(|seat| {
                let sys = Arc::clone(&sys);
                let show = Arc::clone(&show);
                std::thread::spawn(move || {
                    sys.book_tickets(
                        &customer("user"),
                        &show,
                        &[seat.number()],
                        PaymentMethod::Card,
                    )
                })
            })
            .collect();
        let failures: Vec<_> = handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap().err())
            .collect();
        assert!(failures.is_empty(), "all seats should book: {failures:?}");
        assert_eq!(show.available_seats(), 0);
    }

    #[test]
    fn test_cancel_refund() {
        let (sys, show) = seeded_show();
        let user = customer("Alice");
        let (booking, payment) = sys
            .book_tickets(&user, &show, &["A1".to_string()], PaymentMethod::Card)
            .expect("booking succeeds");
        let result = sys.cancel_booking(booking.id, &user).expect("cancels");
        assert_eq!(result.refunded, booking.total_price);
        assert_eq!(
            sys.booking(booking.id).unwrap().status,
            BookingStatus::Cancelled
        );
        let stored = sys.payments.payment(payment.payment_id).unwrap();
        assert_eq!(stored.status, PaymentStatus::Refunded);
        assert_eq!(
            show.seating()
                .into_iter()
                .find(|seat| seat.number() == "A1")
                .unwrap()
                .status(),
            SeatStatus::Available
        );
    }

    #[test]
    fn test_cancel_denied_owner() {
        let (sys, show) = seeded_show();
        let owner = customer("Alice");
        let (booking, _) = sys
            .book_tickets(&owner, &show, &["A1".to_string()], PaymentMethod::Card)
            .expect("booking succeeds");
        let result = sys.cancel_booking(booking.id, &customer("Bob"));
        assert!(matches!(result, Err(BookingError::PermissionDenied)));
    }

    #[test]
    fn test_cancel_frees_seat() {
        let (sys, show) = seeded_show();
        let user = customer("Alice");
        let (booking, _) = sys
            .book_tickets(&user, &show, &["A1".to_string()], PaymentMethod::Card)
            .expect("booking succeeds");
        sys.cancel_booking(booking.id, &user).expect("cancels");
        sys.book_tickets(
            &customer("Bob"),
            &show,
            &["A1".to_string()],
            PaymentMethod::Card,
        )
        .expect("seat is available again");
    }

    #[test]
    fn test_payment_failure_rolls_back() {
        let gateway = Arc::new(MockGateway::default());
        let sys = MovieTicketBookingSystem::with_gateway(Box::new(Arc::clone(&gateway)));
        let admin = admin();
        sys.add_movie(&admin, Movie::new("Dune", "desert", 155))
            .expect("movie added");
        sys.add_theater(&admin, "IMAX", "Delhi")
            .expect("theater added");
        let show = sys
            .add_show(
                &admin,
                "Dune",
                "IMAX",
                chrono::Utc::now().naive_utc(),
                vec![Seat::new("A", 1, SeatType::Premium, 500.0)],
            )
            .expect("show added");

        gateway.fail_next();
        let result = sys.book_tickets(
            &customer("Alice"),
            &show,
            &["A1".to_string()],
            PaymentMethod::Wallet,
        );
        assert!(matches!(result, Err(BookingError::PaymentFailed(_))));
        assert_eq!(
            show.seating()
                .into_iter()
                .find(|seat| seat.number() == "A1")
                .unwrap()
                .status(),
            SeatStatus::Available
        );
    }
}

fn main() {
    run_demo();
    println!("\nAll done.");
}
