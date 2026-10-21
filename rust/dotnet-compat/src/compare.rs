//! `Comparer<T>.Default.Compare` for the priority types the planner uses.

use std::cmp::Ordering;

/// Compares like `Comparer<T>.Default.Compare`, which for numbers is `T.CompareTo`.
///
/// For floating point that differs from IEEE comparison in one way: `double.CompareTo`
/// treats NaN as equal to NaN and smaller than every other value, so it is a total order.
pub trait NetCompare {
    fn net_cmp(&self, other: &Self) -> Ordering;
}

macro_rules! impl_net_compare_ord {
    ($($t:ty),*) => {
        $(impl NetCompare for $t {
            fn net_cmp(&self, other: &Self) -> Ordering {
                self.cmp(other)
            }
        })*
    };
}

impl_net_compare_ord!(i8, i16, i32, i64, u8, u16, u32, u64, usize, isize);

macro_rules! impl_net_compare_float {
    ($($t:ty),*) => {
        $(impl NetCompare for $t {
            // Mirrors Double.CompareTo(double) / Single.CompareTo(float).
            fn net_cmp(&self, other: &Self) -> Ordering {
                if self < other {
                    return Ordering::Less;
                }
                if self > other {
                    return Ordering::Greater;
                }
                if self == other {
                    return Ordering::Equal;
                }
                // At least one of the values is NaN.
                if self.is_nan() {
                    if other.is_nan() { Ordering::Equal } else { Ordering::Less }
                } else {
                    Ordering::Greater
                }
            }
        })*
    };
}

impl_net_compare_float!(f32, f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nan_sorts_first_like_double_compare_to() {
        assert_eq!(f64::NAN.net_cmp(&f64::NEG_INFINITY), Ordering::Less);
        assert_eq!(1.0f64.net_cmp(&f64::NAN), Ordering::Greater);
        assert_eq!(f64::NAN.net_cmp(&f64::NAN), Ordering::Equal);
        assert_eq!((-0.0f64).net_cmp(&0.0), Ordering::Equal);
    }
}
