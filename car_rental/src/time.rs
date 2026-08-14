use std::fmt;

/// Calendar date, range arithmetic only (no wall clocks). Rented nights are
/// the exclusive difference of two dates.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Date {
    pub year: u32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub fn new(year: u32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Days since 1970-01-01 (Howard Hinnant's civil-from-days algorithm).
    fn epoch_day(self) -> i64 {
        let y = self.year as i64 - (self.month <= 2) as i64;
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * (self.month as i64 + if self.month > 2 { -3 } else { 9 }) + 2) / 5
            + self.day as i64
            - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    pub fn add_days(self, days: i64) -> Self {
        let z = self.epoch_day() + days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        Self {
            year: (y + (m <= 2) as i64) as u32,
            month: m as u32,
            day: d as u32,
        }
    }

    /// Full nights between two dates (end is exclusive).
    pub fn nights_from(self, start: Date) -> i64 {
        self.epoch_day() - start.epoch_day()
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_arithmetic() {
        let start = Date::new(2026, 5, 10);
        assert_eq!(start.add_days(3), Date::new(2026, 5, 13));
        assert_eq!(Date::new(2026, 5, 13).nights_from(start), 3);
        assert_eq!(Date::new(2026, 5, 10).nights_from(start), 0);
        assert_eq!(Date::new(2026, 12, 31).add_days(1), Date::new(2027, 1, 1));
        assert_eq!(Date::new(2024, 2, 28).add_days(1), Date::new(2024, 2, 29));
        assert_eq!(Date::new(2026, 3, 1).add_days(-1), Date::new(2026, 2, 28));
    }
}
