-- `venue` and `seats` are both JSON TEXT on this one row. Seat status
-- transitions (Available -> Reserved -> Booked) are done in-place with
-- json_each/json_extract/json_set (see Concert::transition_seat), gated on
-- the seat's current status, so concurrent reserve/book calls can't clobber
-- each other's writes.
CREATE TABLE IF NOT EXISTS
concerts(
    id INTEGER PRIMARY KEY,
    venue TEXT NOT NULL,
    artist TEXT NOT NULL,
    event_date DATETIME NOT NULL,
    seats TEXT NOT NULL
);

-- Named ConcertBooking, not Booking, to avoid colliding with
-- flight-booking's Booking table — migrations are shared across every bin
-- in this crate.
CREATE TABLE IF NOT EXISTS
ConcertBooking(
    id INTEGER PRIMARY KEY,
    concert_id INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    transaction_id INTEGER,
    amount BIGINT,
    refund BIGINT,
    status VARCHAR(16),
    booking_at DATETIME,
    FOREIGN KEY (concert_id) REFERENCES concerts(id)
);
