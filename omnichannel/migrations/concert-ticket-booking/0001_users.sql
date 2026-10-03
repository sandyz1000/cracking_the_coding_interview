-- Users table for concert-ticket-booking. `id` isn't mapped on the `User`
-- struct yet (see review notes) but a real table needs a key regardless.
CREATE TABLE IF NOT EXISTS
Users(
    id INTEGER PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) NOT NULL UNIQUE,
    contact_number VARCHAR(32),
    user_type VARCHAR(16) NOT NULL
);
