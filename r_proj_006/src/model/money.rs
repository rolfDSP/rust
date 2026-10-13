use std::fmt;
use std::ops::{Add, Mul};

/// Amount in cents. A "newtype" around `u64`: the compiler now prevents
/// mixing up money with quantities or IDs, at zero runtime cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Money(u64);

impl Money {
    pub const fn from_cents(cents: u64) -> Self {
        Self(cents)
    }

    pub const fn cents(self) -> u64 {
        self.0
    }
}

// Implementing std traits makes the type feel built-in: `a + b`, `m * 3`, `sum()`.
impl Add for Money {
    type Output = Money;
    fn add(self, rhs: Money) -> Money {
        Money(self.0 + rhs.0)
    }
}

impl Mul<u32> for Money {
    type Output = Money;
    fn mul(self, qty: u32) -> Money {
        Money(self.0 * u64::from(qty))
    }
}

impl std::iter::Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::default(), Add::add)
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02} €", self.0 / 100, self.0 % 100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_display() {
        let total: Money = [Money::from_cents(150), Money::from_cents(250) * 2]
            .into_iter()
            .sum();
        assert_eq!(total, Money::from_cents(650));
        assert_eq!(total.to_string(), "6.50 €");
    }
}
