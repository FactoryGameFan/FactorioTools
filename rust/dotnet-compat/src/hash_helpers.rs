//! `System.Collections.HashHelpers`: the prime capacities `HashSet` and `Dictionary` use.
//!
//! The capacity matters for order in one place: `HashSet`'s copy constructor keeps the
//! source's freed entries only when the source's capacity is small enough (see
//! `NetHashSet::copy_from`). So the capacities have to follow .NET exactly.

/// `HashHelpers.HashPrime`.
const HASH_PRIME: i32 = 101;

/// `HashHelpers.MaxPrimeArrayLength`.
const MAX_PRIME_ARRAY_LENGTH: i32 = 0x7FFF_FFC3;

/// `HashHelpers.Primes`.
const PRIMES: [i32; 72] = [
    3, 7, 11, 17, 23, 29, 37, 47, 59, 71, 89, 107, 131, 163, 197, 239, 293, 353, 431, 521, 631,
    761, 919, 1103, 1327, 1597, 1931, 2333, 2801, 3371, 4049, 4861, 5839, 7013, 8419, 10103, 12143,
    14591, 17519, 21023, 25229, 30293, 36353, 43627, 52361, 62851, 75431, 90523, 108631, 130363,
    156437, 187751, 225307, 270371, 324449, 389357, 467237, 560689, 672827, 807403, 968897,
    1162687, 1395263, 1674319, 2009191, 2411033, 2893249, 3471899, 4166287, 4999559, 5999471,
    7199369,
];

/// `HashHelpers.IsPrime`.
fn is_prime(candidate: i32) -> bool {
    if candidate & 1 != 0 {
        let limit = f64::from(candidate).sqrt() as i32;
        let mut divisor = 3;
        while divisor <= limit {
            if candidate % divisor == 0 {
                return false;
            }
            divisor += 2;
        }
        return true;
    }
    candidate == 2
}

/// `HashHelpers.GetPrime`: the smallest capacity .NET uses that is at least `min`.
pub fn get_prime(min: usize) -> usize {
    let min = i32::try_from(min).expect("capacity overflow");
    for prime in PRIMES {
        if prime >= min {
            return prime as usize;
        }
    }
    // Outside the table: compute, skipping primes that are 1 mod HashPrime.
    let mut i = min | 1;
    while i < i32::MAX {
        if is_prime(i) && (i - 1) % HASH_PRIME != 0 {
            return i as usize;
        }
        i += 2;
    }
    min as usize
}

/// `HashHelpers.ExpandPrime`: the capacity after a resize from `old_size`.
pub fn expand_prime(old_size: usize) -> usize {
    let new_size = 2 * old_size;
    if new_size > MAX_PRIME_ARRAY_LENGTH as usize && (MAX_PRIME_ARRAY_LENGTH as usize) > old_size {
        return MAX_PRIME_ARRAY_LENGTH as usize;
    }
    get_prime(new_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primes_follow_the_dotnet_table() {
        assert_eq!(get_prime(0), 3);
        assert_eq!(get_prime(4), 7);
        assert_eq!(get_prime(7), 7);
        assert_eq!(expand_prime(3), 7);
        assert_eq!(expand_prime(7), 17);
        assert_eq!(expand_prime(17), 37);
    }
}
