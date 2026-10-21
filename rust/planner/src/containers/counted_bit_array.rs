//! `CustomCountedBitArray`, from `OilField/Containers/CustomCountedBitArray.cs`.
//!
//! The default C# build (`USE_BITARRAY`) aliases `CountedBitArray` to
//! `WrapperCountedBitArray`, a wrapper over `System.Collections.BitArray`, while
//! `LocationBitSet` always uses `CustomCountedBitArray`. The two behave the same for every
//! operation the planner calls (CI runs the snapshots under both), so the port has one type
//! and copies `CustomCountedBitArray`, including its index arithmetic.

use std::fmt;

/// A fixed-length bit array that keeps a count of its set bits.
#[derive(Clone, Debug)]
pub struct CountedBitArray {
    array: Vec<i32>,
    count: i32,
    true_count: i32,
}

impl CountedBitArray {
    /// `new CustomCountedBitArray(int length)`.
    pub fn new(length: i32) -> Self {
        let mut int_length = length / 32;
        if length % 32 != 0 {
            int_length += 1;
        }
        CountedBitArray {
            array: vec![0; int_length.max(0) as usize],
            count: length,
            true_count: 0,
        }
    }

    /// `Count`: the number of bits.
    pub fn count(&self) -> i32 {
        self.count
    }

    /// `TrueCount`: the number of set bits.
    pub fn true_count(&self) -> i32 {
        self.true_count
    }

    /// Where the C# indexer looks for bit `index`. It checks only `index >= Count`, then
    /// divides with C#'s truncating `/` and `%`, so `-1` lands on bit 31 of the first int
    /// (the shift count is masked to 5 bits) and anything below `-32` throws from the
    /// array access. The planner never relies on this, but a location just outside the
    /// grid can reach it through `LocationBitSet`, so it is copied rather than tidied up.
    fn locate(&self, index: i32) -> (usize, i32) {
        if index >= self.count {
            panic!("IndexOutOfRangeException: bit {index} of {}", self.count);
        }
        let int_index = index / 32;
        let bit_index = index % 32;
        if int_index < 0 {
            panic!("IndexOutOfRangeException: bit {index} of {}", self.count);
        }
        (int_index as usize, 1i32.wrapping_shl(bit_index as u32 & 31))
    }

    /// The indexer's getter.
    pub fn get(&self, index: i32) -> bool {
        let (int_index, mask) = self.locate(index);
        (self.array[int_index] & mask) != 0
    }

    /// `Set(index, value)`: sets the bit and returns its previous value.
    pub fn set(&mut self, index: i32, value: bool) -> bool {
        let (int_index, mask) = self.locate(index);
        let current_int = self.array[int_index];
        let current = (current_int & mask) != 0;
        if current != value {
            if value {
                self.array[int_index] = current_int | mask;
                self.true_count += 1;
            } else {
                self.array[int_index] = current_int & !mask;
                self.true_count -= 1;
            }
        }
        current
    }

    /// `All(value)`.
    pub fn all(&self, value: bool) -> bool {
        if value {
            self.true_count == self.count
        } else {
            self.true_count == 0
        }
    }

    /// `Any(value)`.
    pub fn any(&self, value: bool) -> bool {
        if value {
            self.true_count > 0
        } else {
            self.true_count < self.count
        }
    }

    /// `SetAll(value)`.
    pub fn set_all(&mut self, value: bool) {
        self.array.fill(if value { -1 } else { 0 });
        if value {
            self.clear_unused_bits();
        }
        self.true_count = if value { self.count } else { 0 };
    }

    /// `And(value)`. Panics like the C#'s `ArgumentException` when the lengths differ.
    pub fn and(&mut self, value: &CountedBitArray) -> &mut Self {
        self.check_same_count(value);
        for (a, b) in self.array.iter_mut().zip(&value.array) {
            *a &= *b;
        }
        self.true_count = self.count_true();
        self
    }

    /// `Or(value)`.
    pub fn or(&mut self, value: &CountedBitArray) -> &mut Self {
        self.check_same_count(value);
        for (a, b) in self.array.iter_mut().zip(&value.array) {
            *a |= *b;
        }
        self.true_count = self.count_true();
        self
    }

    /// `Not()`.
    pub fn not(&mut self) -> &mut Self {
        for a in &mut self.array {
            *a = !*a;
        }
        self.clear_unused_bits();
        self.true_count = self.count - self.true_count;
        self
    }

    /// `GetInt(index)`: one of the backing ints.
    pub fn get_int(&self, index: usize) -> i32 {
        self.array[index]
    }

    fn check_same_count(&self, other: &CountedBitArray) {
        if other.count != self.count {
            panic!("ArgumentException: The two bit arrays must have the same number of elements.");
        }
    }

    fn count_true(&self) -> i32 {
        (0..self.count).filter(|&i| self.get(i)).count() as i32
    }

    fn clear_unused_bits(&mut self) {
        if self.count % 32 == 0 {
            return;
        }
        let last = self.array.len() - 1;
        let mut current = self.array[last];
        if current == 0 {
            return;
        }
        for i in self.count % 32..32 {
            current &= !(1i32 << i);
        }
        self.array[last] = current;
    }
}

/// `Equals(CustomCountedBitArray)`: same length and same backing ints.
impl PartialEq for CountedBitArray {
    fn eq(&self, other: &Self) -> bool {
        self.count == other.count && self.array == other.array
    }
}

impl Eq for CountedBitArray {}

/// `ToString()`: one `0` or `1` per bit.
impl fmt::Display for CountedBitArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for i in 0..self.count {
            f.write_str(if self.get(i) { "1" } else { "0" })?;
        }
        Ok(())
    }
}

/// Ported from `test/FactorioTools.Test/OilField/CustomCountedBitArrayTest.cs`.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clears_upper_bits_in_set_all() {
        let mut bits = CountedBitArray::new(100);
        bits.set_all(true);
        for i in 0..bits.count() {
            bits.set(i, false);
        }
        assert_eq!(bits.get_int(3), 0);
    }

    #[test]
    fn clears_upper_bits_in_not() {
        let mut bits = CountedBitArray::new(100);
        bits.not();
        for i in 0..bits.count() {
            bits.set(i, false);
        }
        assert_eq!(bits.get_int(3), 0);
    }

    #[test]
    fn clears_only_upper_unused_bits() {
        for count in [1, 2, 31, 32, 33, 63, 64, 65, 100] {
            let mut a = CountedBitArray::new(count);
            for i in 0..a.count() {
                a.set(i, true);
            }
            let mut b = CountedBitArray::new(count);
            b.not();
            let mut last = count / 32;
            if count % 32 == 0 {
                last -= 1;
            }
            assert_eq!(
                b.get_int(last as usize),
                a.get_int(last as usize),
                "count {count}"
            );
        }
    }

    #[test]
    fn logical_equals_not_equal() {
        let mut a = CountedBitArray::new(100);
        a.set(50, true);
        let mut b = CountedBitArray::new(100);
        b.set(49, true);
        assert_ne!(a, b);
    }

    #[test]
    fn logical_equals_different_count() {
        let mut a = CountedBitArray::new(100);
        a.set(50, true);
        let mut b = CountedBitArray::new(99);
        b.set(50, true);
        assert_ne!(a, b);
    }

    #[test]
    fn logical_equals_equal() {
        let mut a = CountedBitArray::new(100);
        a.set(50, true);
        let mut b = CountedBitArray::new(100);
        b.set(50, true);
        assert_eq!(a, b);
    }

    #[test]
    fn logical_equals_same() {
        let a = CountedBitArray::new(100);
        assert_eq!(a, a.clone());
    }

    #[test]
    fn counts_and_logic() {
        let mut a = CountedBitArray::new(40);
        assert!(!a.set(3, true));
        assert!(a.set(3, true));
        a.set(39, true);
        assert_eq!(a.true_count(), 2);
        let mut b = a.clone();
        b.not();
        assert_eq!(b.true_count(), 38);
        b.and(&a);
        assert_eq!(b.true_count(), 0);
        b.or(&a);
        assert_eq!(b.true_count(), 2);
        assert!(b.any(true) && b.any(false) && !b.all(true));
    }

    #[test]
    fn minus_one_reads_bit_31_like_the_csharp() {
        let mut a = CountedBitArray::new(64);
        a.set(31, true);
        assert!(a.get(-1));
    }
}
