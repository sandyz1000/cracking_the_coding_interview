//! Currency value types. See DESIGN.md.

use std::fmt;

use crate::error::{WalletError, WalletResult};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Currency {
    Usd,
    Eur,
    Gbp,
    Inr,
}

impl Currency {
    /// Minor units per one USD minor unit, scaled by 1000.
    const fn usd_minor_rate(self) -> i128 {
        // Exhaustive so a new currency cannot compile without a rate.
        match self {
            Currency::Usd => 1_000,
            Currency::Eur => 900,
            Currency::Gbp => 1_280,
            Currency::Inr => 83_000,
        }
    }

    /// Minor units in one major unit.
    pub const fn minor_per_major(self) -> i64 {
        match self {
            Currency::Usd | Currency::Eur | Currency::Gbp | Currency::Inr => 100,
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Currency::Usd => "USD",
            Currency::Eur => "EUR",
            Currency::Gbp => "GBP",
            Currency::Inr => "INR",
        };
        write!(w, "{code}")
    }
}

/// An amount in one currency's minor units.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Money {
    pub amount: i64,
    pub currency: Currency,
}

impl Money {
    pub const fn minor_units(amount: i64, currency: Currency) -> Self {
        Self { amount, currency }
    }

    /// `Money::from_parts(100, 50, Currency::Usd)` is $100.50.
    pub const fn from_parts(whole: i64, fraction: i64, currency: Currency) -> Self {
        Self {
            amount: whole * currency.minor_per_major() + fraction,
            currency,
        }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, w: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scale = self.currency.minor_per_major() as u64;
        let sign = if self.amount < 0 { "-" } else { "" };
        let abs = self.amount.unsigned_abs();
        let width = scale.ilog10() as usize;
        write!(
            w,
            "{} {sign}{}.{:0width$}",
            self.currency,
            abs / scale,
            abs % scale,
        )
    }
}

/// Cross-currency conversion pivoted through USD. See DESIGN.md.
pub struct CurrencyConverter;

impl CurrencyConverter {
    pub fn convert(amount: &Money, to: Currency) -> WalletResult<Money> {
        if amount.currency == to {
            return Ok(*amount);
        }
        // i128 interior so amount * to_rate cannot wrap before the divide.
        let value = (amount.amount as i128)
            .checked_mul(to.usd_minor_rate())
            .map(|v| v / amount.currency.usd_minor_rate())
            .and_then(|v| i64::try_from(v).ok())
            .ok_or_else(|| {
                WalletError::ConversionOverflow(format!("{} -> {}", amount.currency, to))
            })?;
        Ok(Money {
            amount: value,
            currency: to,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_conversion() {
        let m = Money::from_parts(10, 0, Currency::Usd);
        assert_eq!(CurrencyConverter::convert(&m, Currency::Usd).unwrap(), m);
    }

    #[test]
    fn test_usd_eur() {
        let out =
            CurrencyConverter::convert(&Money::from_parts(10, 0, Currency::Usd), Currency::Eur)
                .unwrap();
        assert_eq!(out, Money::from_parts(9, 0, Currency::Eur));
    }

    #[test]
    fn test_eur_usd() {
        let back =
            CurrencyConverter::convert(&Money::from_parts(9, 0, Currency::Eur), Currency::Usd)
                .unwrap();
        assert_eq!(back, Money::from_parts(10, 0, Currency::Usd));
    }

    #[test]
    fn test_usd_inr() {
        let out =
            CurrencyConverter::convert(&Money::from_parts(10, 0, Currency::Usd), Currency::Inr)
                .unwrap();
        assert_eq!(out, Money::from_parts(830, 0, Currency::Inr));
    }

    #[test]
    fn test_convert_overflow() {
        let huge = Money::minor_units(i64::MAX, Currency::Usd);
        assert!(matches!(
            CurrencyConverter::convert(&huge, Currency::Inr),
            Err(WalletError::ConversionOverflow(_))
        ));
    }

    #[test]
    fn test_display_format() {
        assert_eq!(
            Money::from_parts(100, 50, Currency::Usd).to_string(),
            "USD 100.50"
        );
        assert_eq!(
            Money::minor_units(-5, Currency::Eur).to_string(),
            "EUR -0.05"
        );
    }
}
