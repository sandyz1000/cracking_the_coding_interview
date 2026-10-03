-- Aircraft table
CREATE TABLE IF NOT EXISTS
Aircraft(
    id INTEGER PRIMARY KEY,
    tail_no VARCHAR(8),
    total_seats INT,
    model_no VARCHAR(16)
);

-- Flight table: source/destination/crew stored as JSON text; seats are
-- normalized into flight_seats so a reservation can be a conditional
-- UPDATE on a single row instead of a read-modify-write of a JSON blob.
CREATE TABLE IF NOT EXISTS
Flight(
    id INTEGER PRIMARY KEY,
    aircraft_id INTEGER,
    flight_no VARCHAR(8) UNIQUE,
    source TEXT,
    destination TEXT,
    fare BIGINT,
    departure_time DATETIME,
    arrival_time DATETIME,
    status VARCHAR(16),
    crew TEXT,
    FOREIGN KEY (aircraft_id) REFERENCES Aircraft(id)
);

-- One row per seat. `status` gates reservation: `UPDATE ... WHERE status =
-- 'AVAILABLE'` is the atomic seat-claim; a rows_affected() mismatch means
-- someone else got there first.
CREATE TABLE IF NOT EXISTS
flight_seats(
    flight_id INTEGER NOT NULL,
    seat_no VARCHAR(8) NOT NULL,
    seat_type VARCHAR(16) NOT NULL,
    status VARCHAR(16) NOT NULL,
    holder INTEGER,
    PRIMARY KEY (flight_id, seat_no),
    FOREIGN KEY (flight_id) REFERENCES Flight(id)
);

-- Booking table
CREATE TABLE IF NOT EXISTS
Booking(
    id INTEGER PRIMARY KEY,
    transaction_id INTEGER,
    flight_no VARCHAR(8),
    passenger_id INTEGER,
    seat TEXT,
    amount BIGINT,
    refund BIGINT,
    status VARCHAR,
    baggage TEXT,
    booking_date DATETIME
);
