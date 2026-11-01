//! CLDR plural categories for the interface languages (ADR-0009).
//!
//! The first strings with a number come with TASK-9 and TASK-13.
#![cfg_attr(not(test), expect(dead_code))]

use super::Lang;

/// A CLDR plural category; the language decides which ones it uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    One,
    Few,
    Many,
    Other,
}

/// The CLDR operands of a number: `i` is the integer part, `v` the count of visible
/// fraction digits (so "1.50" has v = 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Operands {
    i: u64,
    v: u32,
}

impl From<u64> for Operands {
    fn from(n: u64) -> Self {
        Self { i: n, v: 0 }
    }
}

impl Operands {
    /// Parses a decimal written with a dot, such as "1.5"; the digits after the dot are
    /// kept as written, because CLDR treats "1" and "1.0" differently.
    pub fn from_decimal(text: &str) -> Option<Self> {
        let (int, frac) = text.split_once('.').unwrap_or((text, ""));
        let all_digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
        if int.is_empty() || !all_digits(int) || !all_digits(frac) {
            return None;
        }
        Some(Self {
            i: int.parse().ok()?,
            v: u32::try_from(frac.len()).ok()?,
        })
    }
}

/// The plural category of `n` in `lang`, by the CLDR rules.
pub fn category(lang: Lang, n: Operands) -> Category {
    let Operands { i, v } = n;
    match lang {
        Lang::En if i == 1 && v == 0 => Category::One,
        Lang::En => Category::Other,
        Lang::Uk if v != 0 => Category::Other,
        Lang::Uk => match (i % 10, i % 100) {
            (1, m) if m != 11 => Category::One,
            (2..=4, m) if !(12..=14).contains(&m) => Category::Few,
            _ => Category::Many,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Examples from the CLDR chart (language_plural_rules.html), one per row.
    #[test]
    fn ukrainian_integers_follow_cldr() {
        let cases = [
            (Category::One, &[1u64, 21, 31, 101, 1001][..]),
            (Category::Few, &[2, 3, 4, 22, 24, 102, 1002]),
            (
                Category::Many,
                &[0, 5, 9, 10, 11, 12, 14, 19, 20, 100, 111, 1000],
            ),
        ];
        for (expected, numbers) in cases {
            for &n in numbers {
                assert_eq!(category(Lang::Uk, n.into()), expected, "uk {n}");
            }
        }
    }

    #[test]
    fn ukrainian_decimals_are_other() {
        for n in ["1.5", "0.0", "1.0", "2.5", "10.1"] {
            let operands = Operands::from_decimal(n).unwrap();
            assert_eq!(category(Lang::Uk, operands), Category::Other, "uk {n}");
        }
    }

    #[test]
    fn english_has_one_only_for_integer_one() {
        assert_eq!(category(Lang::En, 1.into()), Category::One);
        for n in [0u64, 2, 11, 21, 101] {
            assert_eq!(category(Lang::En, n.into()), Category::Other, "en {n}");
        }
        let one_point_zero = Operands::from_decimal("1.0").unwrap();
        assert_eq!(category(Lang::En, one_point_zero), Category::Other);
    }

    #[test]
    fn decimal_parsing_rejects_garbage() {
        assert_eq!(Operands::from_decimal("abc"), None);
        assert_eq!(Operands::from_decimal(""), None);
    }
}
