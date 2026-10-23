//! `FromField` for the value types that ship: text, numbers, `bool`,
//! `char`, `Option`, `Vec`.

use alloc::string::String;
use alloc::vec::Vec;

use super::{FileValue, FromField, ValueError};

/// A scalar slot: a second value is `Duplicate`.
fn set<T>(slot: &mut Option<T>, value: T) -> Result<(), ValueError> {
    if slot.is_some() {
        return Err(ValueError::Duplicate);
    }
    *slot = Some(value);
    Ok(())
}

impl FromField for String {
    type Slot = Option<String>;

    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        set(slot, String::from(value))
    }

    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        slot.ok_or(ValueError::Missing)
    }
}

/// ASCII digits only, at least one.
fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

macro_rules! unsigned {
    ($($t:ty),*) => {$(
        impl FromField for $t {
            type Slot = Option<$t>;

            fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
                if !digits(value) {
                    return Err(ValueError::Invalid("not a whole number"));
                }
                let n = value.parse::<$t>().map_err(|_| ValueError::Invalid("out of range"))?;
                set(slot, n)
            }

            fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
                slot.ok_or(ValueError::Missing)
            }
        }
    )*};
}

macro_rules! signed {
    ($($t:ty),*) => {$(
        impl FromField for $t {
            type Slot = Option<$t>;

            fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
                if !digits(value.strip_prefix('-').unwrap_or(value)) {
                    return Err(ValueError::Invalid("not a whole number"));
                }
                let n = value.parse::<$t>().map_err(|_| ValueError::Invalid("out of range"))?;
                set(slot, n)
            }

            fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
                slot.ok_or(ValueError::Missing)
            }
        }
    )*};
}

unsigned!(u8, u16, u32, u64, u128, usize);
signed!(i8, i16, i32, i64, i128, isize);

macro_rules! float {
    ($($t:ty),*) => {$(
        impl FromField for $t {
            type Slot = Option<$t>;

            fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
                let n = value.parse::<$t>().map_err(|_| ValueError::Invalid("not a number"))?;
                if !n.is_finite() {
                    return Err(ValueError::Invalid("not a finite number"));
                }
                set(slot, n)
            }

            fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
                slot.ok_or(ValueError::Missing)
            }
        }
    )*};
}

float!(f32, f64);

impl FromField for bool {
    type Slot = Option<bool>;

    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        let b = match value {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            _ => return Err(ValueError::Invalid("not on, true, 1, off, false, or 0")),
        };
        set(slot, b)
    }

    /// An unchecked checkbox sends nothing: missing is `false`.
    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        Ok(slot.unwrap_or(false))
    }
}

impl FromField for char {
    type Slot = Option<char>;

    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        let mut chars = value.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => set(slot, c),
            _ => Err(ValueError::Invalid("not exactly one character")),
        }
    }

    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        slot.ok_or(ValueError::Missing)
    }
}

impl<T: FromField> FromField for Option<T> {
    type Slot = (bool, T::Slot);

    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        slot.0 = true;
        T::accept_text(&mut slot.1, value)
    }

    fn accept_file(slot: &mut Self::Slot, file: FileValue<'_>) -> Result<(), ValueError> {
        slot.0 = true;
        T::accept_file(&mut slot.1, file)
    }

    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        if !slot.0 {
            return Ok(None);
        }
        match T::finish(slot.1) {
            Ok(v) => Ok(Some(v)),
            // A file input left empty arrives but holds nothing.
            Err(ValueError::Missing) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl<T: FromField> FromField for Vec<T> {
    type Slot = Vec<T>;

    fn accept_text(slot: &mut Self::Slot, value: &str) -> Result<(), ValueError> {
        let mut one = T::Slot::default();
        T::accept_text(&mut one, value)?;
        slot.push(T::finish(one)?);
        Ok(())
    }

    fn accept_file(slot: &mut Self::Slot, file: FileValue<'_>) -> Result<(), ValueError> {
        let mut one = T::Slot::default();
        T::accept_file(&mut one, file)?;
        match T::finish(one) {
            Ok(v) => slot.push(v),
            Err(ValueError::Missing) => {}
            Err(e) => return Err(e),
        }
        Ok(())
    }

    fn finish(slot: Self::Slot) -> Result<Self, ValueError> {
        Ok(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one<T: FromField>(values: &[&str]) -> Result<T, ValueError> {
        let mut slot = T::Slot::default();
        for v in values {
            T::accept_text(&mut slot, v)?;
        }
        T::finish(slot)
    }

    #[test]
    fn test_integers_strict() {
        assert_eq!(one::<u8>(&["255"]), Ok(255));
        assert_eq!(one::<u8>(&["007"]), Ok(7));
        assert_eq!(
            one::<u8>(&["256"]),
            Err(ValueError::Invalid("out of range"))
        );
        for bad in ["+1", " 1", "1 ", "", "-0", "1.0", "0x1"] {
            assert!(one::<u8>(&[bad]).is_err(), "{bad:?}");
        }
        assert_eq!(one::<i8>(&["-128"]), Ok(-128));
        assert_eq!(
            one::<i8>(&["-129"]),
            Err(ValueError::Invalid("out of range"))
        );
        assert!(
            one::<i8>(&["+1"]).is_err()
                && one::<i8>(&["-"]).is_err()
                && one::<i8>(&["--1"]).is_err()
        );
        assert_eq!(one::<u64>(&[]), Err(ValueError::Missing));
        assert_eq!(one::<u64>(&["1", "2"]), Err(ValueError::Duplicate));
        assert_eq!(
            one::<u128>(&["340282366920938463463374607431768211455"]),
            Ok(u128::MAX)
        );
    }

    #[test]
    fn test_floats_refuse_non_finite() {
        assert_eq!(one::<f64>(&["1e308"]), Ok(1e308));
        assert_eq!(one::<f64>(&["-0"]), Ok(-0.0));
        for bad in ["1e309", "NaN", "inf", "-inf", "infinity", "", "abc"] {
            assert!(one::<f64>(&[bad]).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn test_bool_spellings_and_missing() {
        for (s, v) in [
            ("on", true),
            ("true", true),
            ("1", true),
            ("off", false),
            ("false", false),
            ("0", false),
        ] {
            assert_eq!(one::<bool>(&[s]), Ok(v));
        }
        assert!(one::<bool>(&["ON"]).is_err() && one::<bool>(&["yes"]).is_err());
        assert_eq!(one::<bool>(&[]), Ok(false));
        assert_eq!(one::<bool>(&["on", "on"]), Err(ValueError::Duplicate));
    }

    #[test]
    fn test_char_option_vec() {
        assert_eq!(one::<char>(&["é"]), Ok('é'));
        assert!(one::<char>(&[""]).is_err() && one::<char>(&["ab"]).is_err());
        assert_eq!(one::<Option<u8>>(&[]), Ok(None));
        assert_eq!(one::<Option<u8>>(&["3"]), Ok(Some(3)));
        assert_eq!(one::<Option<u8>>(&["3", "4"]), Err(ValueError::Duplicate));
        assert_eq!(one::<Vec<u8>>(&[]), Ok(Vec::new()));
        assert_eq!(one::<Vec<u8>>(&["1", "2", "3"]), Ok(alloc::vec![1, 2, 3]));
        assert!(one::<Vec<u8>>(&["1", "x"]).is_err());
        assert_eq!(one::<String>(&["a"]), Ok(String::from("a")));
    }

    #[test]
    fn test_text_into_file_field_is_wrong_kind() {
        let mut slot = <super::super::FileData as FromField>::Slot::default();
        assert_eq!(
            <super::super::FileData as FromField>::accept_text(&mut slot, "x"),
            Err(ValueError::ExpectedFile)
        );
    }
}
