//! The hasher behind [`NetHashSet`](crate::NetHashSet) and
//! [`NetDictionary`](crate::NetDictionary)'s key-to-entry index.
//!
//! The index is only ever looked up, never enumerated, so its hash function cannot change
//! any output. std's default hasher is SipHash with a random seed per map, which is built
//! to resist hash flooding and is slow for small keys. The planner's keys are grid
//! locations from the user's own blueprint, planned in their own browser, so flooding is
//! not a concern there. This is the FxHash mix (rotate, xor, multiply) used by rustc, on
//! whole words. It has no seed, so runs are also repeatable.

use std::hash::{BuildHasherDefault, Hasher};

/// FxHash's multiplier for 64-bit words.
const SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

#[derive(Clone, Copy, Default)]
pub struct FastHasher(u64);

impl FastHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(SEED);
    }
}

impl Hasher for FastHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }

    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (words, rest) = bytes.as_chunks::<8>();
        for &word in words {
            self.add(u64::from_le_bytes(word));
        }
        if !rest.is_empty() {
            let mut word = [0u8; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(word));
        }
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }

    #[inline]
    fn write_i32(&mut self, i: i32) {
        self.add(u64::from(i as u32));
    }
}

/// Builds [`FastHasher`]s, for `HashMap::with_hasher`.
pub type FastBuildHasher = BuildHasherDefault<FastHasher>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{BuildHasher, Hash};

    fn hash<T: Hash>(value: T) -> u64 {
        FastBuildHasher::default().hash_one(value)
    }

    #[test]
    fn is_fixed_and_tells_small_keys_apart() {
        // No seed: the same key hashes the same in every map and every run.
        assert_eq!(hash((3i32, 4i32)), hash((3i32, 4i32)));
        // Neighbouring grid cells, and swapped coordinates, get different hashes.
        let mut seen = std::collections::HashSet::new();
        for x in -20i32..20 {
            for y in -20i32..20 {
                assert!(seen.insert(hash((x, y))), "collision at ({x}, {y})");
            }
        }
    }

    #[test]
    fn byte_writes_cover_a_partial_last_word() {
        let mut a = FastHasher::default();
        a.write(b"0123456789");
        let mut b = FastHasher::default();
        b.write(b"0123456788");
        assert_ne!(a.finish(), b.finish());
    }
}
