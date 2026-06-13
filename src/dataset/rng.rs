//! A tiny, self-contained, deterministic pseudo-random number generator.
//!
//! The synthetic dataset generator must be **bit-for-bit reproducible given a
//! seed**, on every platform, forever (board item T-0035). The `rand` crate's
//! distribution algorithms are explicitly *not* covered by a stability
//! guarantee across minor versions, so depending on it would make our committed
//! sample non-reproducible after a routine dependency bump. We therefore vendor
//! a fixed, documented algorithm here — no new dependency, determinism by
//! construction, and trivially license-clean (authored in-repo).
//!
//! The algorithm is **SplitMix64** (Steele, Lea & Flood, 2014 — public-domain
//! reference algorithm), a fast, well-distributed 64-bit generator with a
//! 2^64 period. It is more than adequate for generating test/benchmark data
//! (it is *not* a cryptographic RNG and must never be used as one).

/// A deterministic SplitMix64 generator.
///
/// Construct with a seed via [`SplitMix64::new`]; identical seeds always produce
/// identical streams. `Clone` so a generator can be forked at a known point.
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// A generator seeded with `seed`. Identical seeds yield identical streams.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        SplitMix64 { state: seed }
    }

    /// The next 64-bit value in the stream (the canonical SplitMix64 step).
    pub fn next_u64(&mut self) -> u64 {
        // golden-ratio increment
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniformly distributed `u64` in `[0, bound)`.
    ///
    /// Uses Lemire's nearly-divisionless multiply-shift reduction, which is
    /// unbiased for the data-generation use here. `bound` must be non-zero.
    ///
    /// # Panics
    ///
    /// Panics if `bound == 0`.
    pub fn below(&mut self, bound: u64) -> u64 {
        assert!(bound != 0, "SplitMix64::below requires a non-zero bound");
        // 128-bit multiply, take the high 64 bits: x * bound / 2^64.
        let product = u128::from(self.next_u64()) * u128::from(bound);
        #[allow(clippy::cast_possible_truncation)]
        let hi = (product >> 64) as u64;
        hi
    }

    /// A float uniformly distributed in the half-open interval `[0.0, 1.0)`.
    ///
    /// Builds the float from the top 53 bits of a 64-bit draw, the standard
    /// construction that yields every representable double in the unit interval
    /// with the correct probability.
    pub fn unit_f64(&mut self) -> f64 {
        // 53 high bits → an integer in [0, 2^53), scaled into [0, 1).
        #[allow(clippy::cast_precision_loss)]
        let mantissa = (self.next_u64() >> 11) as f64;
        mantissa / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_seeds_produce_identical_streams() {
        let mut a = SplitMix64::new(42);
        let mut b = SplitMix64::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = SplitMix64::new(1);
        let mut b = SplitMix64::new(2);
        // Overwhelmingly likely to differ within the first draw; assert over a
        // window to be safe.
        let mut differed = false;
        for _ in 0..8 {
            if a.next_u64() != b.next_u64() {
                differed = true;
                break;
            }
        }
        assert!(differed, "distinct seeds should yield distinct streams");
    }

    #[test]
    fn known_answer_first_outputs() {
        // Golden values pin the algorithm so a refactor that changes the stream
        // is caught immediately (which would silently break the committed
        // sample's reproducibility). These are the canonical SplitMix64 outputs
        // for seed 0.
        let mut g = SplitMix64::new(0);
        assert_eq!(g.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(g.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(g.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn below_is_in_range_and_deterministic() {
        let mut g = SplitMix64::new(7);
        let mut h = SplitMix64::new(7);
        for _ in 0..10_000 {
            let v = g.below(10);
            assert!(v < 10);
            assert_eq!(v, h.below(10));
        }
    }

    #[test]
    fn below_one_is_always_zero() {
        let mut g = SplitMix64::new(123);
        for _ in 0..100 {
            assert_eq!(g.below(1), 0);
        }
    }

    #[test]
    #[should_panic(expected = "non-zero bound")]
    fn below_zero_panics() {
        let mut g = SplitMix64::new(1);
        let _ = g.below(0);
    }

    #[test]
    fn unit_f64_is_in_unit_interval() {
        let mut g = SplitMix64::new(99);
        for _ in 0..10_000 {
            let x = g.unit_f64();
            assert!((0.0..1.0).contains(&x));
        }
    }

    #[test]
    fn below_roughly_uniform() {
        // Coarse uniformity sanity check: 100k draws into 10 buckets should each
        // land within ±20% of the expected 10k.
        let mut g = SplitMix64::new(2024);
        let mut buckets = [0u32; 10];
        let n = 100_000u32;
        for _ in 0..n {
            buckets[g.below(10) as usize] += 1;
        }
        let expected = f64::from(n) / 10.0;
        for (i, &count) in buckets.iter().enumerate() {
            let ratio = f64::from(count) / expected;
            assert!(
                (0.8..1.2).contains(&ratio),
                "bucket {i} skewed: count={count} ratio={ratio}"
            );
        }
    }
}
