//! Discrete-event cold-start latency simulation for the caerostris-db
//! selectivity-envelope theorem.
//!
//! # What this proves
//!
//! This is the **Cat. 11 latency-model evidence** (and the simulation half of
//! the Cat. 3 GATE) called for by `docs/process/formal-verification-policy.md`
//! Artifact 2, and the deliverable of board item **T-0014**. It corroborates the
//! analytical cost model ratified in **ADR-0001**
//! (`docs/adr/0001-latency-selectivity-envelope.md`) by Monte-Carlo: it samples
//! per-request S3 GET latencies from a lognormal distribution and assembles them
//! into the *exact* serial/parallel phase structure of a cold-start 6-hop
//! unanchored property-filtered `MATCH ... LIMIT 10`, then reports the
//! end-to-end P99.
//!
//! # The model (verbatim from the ratified artifacts)
//!
//! Total cold-start query latency for one trial:
//!
//! ```text
//! T_total = T_lat + T_transfer + T_compute
//!
//!   T_lat      = Σ_{k=1..K} max(L_{k,1}, .., L_{k,M_k})     // per-phase max-of-M
//!   T_transfer = B_query / W                                 // byte transfer
//!   T_compute  = fixed budget                                // deserialization etc.
//! ```
//!
//! * `K` is the **serial phase depth** (`K_min = 1 manifest + 1 index + 6 hops`
//!   at `r = 1` ⇒ **8**) — SPIKE-0006 / decision 0010. Each phase's GETs cannot
//!   be issued until the previous phase returns (hop `k+1`'s frontier is unknown
//!   until hop `k` returns), so the phases are strictly serial.
//! * Each phase issues `M_k` **parallel** range-GETs; the phase completes at the
//!   **max** of its `M_k` GET latencies — the intra-phase order-statistic tail
//!   from **BUG-0004 / decision 0005**. The *bare* serial floor `K · L_p99` is
//!   the `M_k = 1` case; `max-of-M` amplifies it. Both terms are surfaced in
//!   [`SimReport`].
//! * `L_p99` is the per-request S3 P99 (design point **50 ms**, ADR §1.1) and the
//!   GET distribution is **lognormal** fitted from `(P50, P99)` (ADR §3.4).
//! * `B_query ≤ B_max` for an in-envelope query; `B_max` is derived from the
//!   byte budget `W · (T_budget − T_lat − T_compute)` (ADR §1.2 / §1.7).
//!
//! Everything here is from S3 reads only — there is **no cache term**. The
//! simulation is therefore structurally cache-independent, matching the
//! non-negotiable invariant that the cold-start SLA holds with the cache off.
//!
//! # Determinism
//!
//! All randomness comes from a seeded SplitMix64 generator, so every percentile
//! is reproducible in CI from `(seed, trials)`. No external dependencies.

#![forbid(unsafe_code)]

/// 99th-percentile z-score for a standard normal (`Φ⁻¹(0.99)`), used to fit a
/// lognormal from its P50/P99.
const Z99: f64 = 2.326_347_874_040_841_5;

/// A deterministic [SplitMix64] pseudo-random generator.
///
/// Chosen for its excellent statistical quality from a single 64-bit state and
/// its triviality to implement with zero dependencies. Determinism (fixed seed
/// ⇒ fixed stream) is what makes the SLA assertions reproducible in CI.
///
/// [SplitMix64]: https://prng.di.unimi.it/splitmix64.c
#[derive(Debug, Clone)]
pub struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    /// Seed the generator. Any `u64` is a valid seed.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Next raw `u64`.
    pub fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Next uniform `f64` in the half-open interval `[0, 1)`.
    ///
    /// Uses the top 53 bits (the f64 mantissa width) for a uniform draw.
    pub fn next_f64(&mut self) -> f64 {
        // 53-bit mantissa ⇒ divide by 2^53.
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A standard-normal sample via the Box–Muller transform.
    ///
    /// We clamp the uniform away from 0 so `ln(u1)` is finite.
    pub fn next_standard_normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// A lognormal per-request latency distribution, fitted from two percentiles.
///
/// For `ln X ~ N(μ, σ²)`: the median is `exp(μ)`, so `μ = ln(P50)`, and the 99th
/// percentile is `exp(μ + σ·z99)`, so `σ = (ln(P99) − ln(P50)) / z99`. This is
/// the distribution family the ADR's max-of-M α factors were calibrated against
/// (ADR §1.7 / decision 0005).
#[derive(Debug, Clone, Copy)]
pub struct LatencyDist {
    /// Mean of the underlying normal (`ln` of the median latency in ms).
    pub mu: f64,
    /// Std-dev of the underlying normal.
    pub sigma: f64,
}

impl LatencyDist {
    /// Fit a lognormal from a (P50, P99) pair, both in milliseconds.
    ///
    /// # Panics
    /// Panics if `p50 <= 0`, `p99 <= 0`, or `p99 < p50` — these are not valid
    /// latency percentiles and indicate a calibration error.
    #[must_use]
    pub fn lognormal_from_p50_p99(p50_ms: f64, p99_ms: f64) -> Self {
        assert!(p50_ms > 0.0, "p50 must be > 0");
        assert!(p99_ms > 0.0, "p99 must be > 0");
        assert!(p99_ms >= p50_ms, "p99 must be >= p50");
        let mu = p50_ms.ln();
        let sigma = (p99_ms.ln() - mu) / Z99;
        Self { mu, sigma }
    }

    /// Draw one latency sample (milliseconds) from the fitted distribution.
    pub fn sample_ms(&self, rng: &mut SplitMix64) -> f64 {
        (self.mu + self.sigma * rng.next_standard_normal()).exp()
    }

    /// The closed-form median latency in milliseconds (`exp(μ)`).
    #[must_use]
    pub fn median_ms(&self) -> f64 {
        self.mu.exp()
    }

    /// The closed-form P99 latency in milliseconds (`exp(μ + σ·z99)`).
    #[must_use]
    pub fn p99_ms(&self) -> f64 {
        (self.mu + self.sigma * Z99).exp()
    }
}

/// Network bandwidth in bytes per second.
///
/// The two cases the theorem must cover: 1 Gbps (relaxed byte budget) and the
/// **binding** 50 Mbps case (commander's intent: "50 Mbps is the binding
/// constraint; always include it").
#[derive(Debug, Clone, Copy)]
pub struct Bandwidth {
    /// Bytes per second.
    pub bytes_per_s: f64,
    /// Human-readable label for reports.
    pub label: &'static str,
}

impl Bandwidth {
    /// 1 Gbps = 125 MB/s (ADR §1.1).
    #[must_use]
    pub const fn gbps_1() -> Self {
        Self {
            bytes_per_s: 125_000_000.0,
            label: "1 Gbps",
        }
    }

    /// 50 Mbps = 6.25 MB/s (ADR §1.1) — the binding case.
    #[must_use]
    pub const fn mbps_50() -> Self {
        Self {
            bytes_per_s: 6_250_000.0,
            label: "50 Mbps",
        }
    }
}

/// The ratified envelope parameters (ADR-0001 §1.1, SPIKE-0006 §4).
#[derive(Debug, Clone, Copy)]
pub struct EnvelopeParams {
    /// Serial phase depth `K_min` (8 at r=1: 1 manifest + 1 index + 6 hops).
    pub k_min: usize,
    /// Per-request S3 P99 latency (design point, ms). The serial floor is
    /// `k_min * l_p99_ms`.
    pub l_p99_ms: f64,
    /// Frontier-width cap: max parallel GETs per phase (ADR §1.7).
    pub m_max: usize,
    /// Fixed compute budget per query (ms) — deserialization, predicate eval,
    /// LIMIT tracking (ADR §1.6).
    pub t_compute_ms: f64,
    /// End-to-end P99 target (ms) — the 1 s SLA.
    pub t_target_ms: f64,
    /// End-to-end P99 hard ceiling (ms) — the 2 s ceiling.
    pub t_ceiling_ms: f64,
    /// The max-of-M order-statistic amplification factor α(M_max) used by the
    /// *analytical* cross-check (ADR §1.7 table; α = 1.10 at M_max = 8).
    pub alpha: f64,
}

impl EnvelopeParams {
    /// The ratified design point: r=1 ⇒ K_min=8, L_p99=50 ms, M_max=8 ⇒ α=1.10,
    /// T_compute=100 ms, target 1 s, ceiling 2 s (ADR §1.1 / §1.7).
    #[must_use]
    pub const fn design_point() -> Self {
        Self {
            k_min: 8,
            l_p99_ms: 50.0,
            m_max: 8,
            t_compute_ms: 100.0,
            t_target_ms: 1000.0,
            t_ceiling_ms: 2000.0,
            alpha: 1.10,
        }
    }

    /// The bare serial latency floor `K_min · L_p99` (ms) — the `M_k = 1` case,
    /// reported as a distinct line item per SPIKE-0006 §6.
    #[must_use]
    pub fn serial_floor_ms(&self) -> f64 {
        self.k_min as f64 * self.l_p99_ms
    }

    /// `B_max` (bytes) at the given bandwidth and SLA budget, using the α-aware
    /// latency reserve (ADR §1.7):
    /// `B_max = W · (T_budget − K·L_p99·α − T_compute)`.
    ///
    /// Returns 0 if the latency reserve alone already exhausts the budget (a
    /// "deployment too slow" condition).
    #[must_use]
    pub fn b_max_bytes(&self, bw: Bandwidth, budget_ms: f64) -> f64 {
        let reserve_ms = self.serial_floor_ms() * self.alpha + self.t_compute_ms;
        let usable_ms = budget_ms - reserve_ms;
        if usable_ms <= 0.0 {
            return 0.0;
        }
        bw.bytes_per_s * (usable_ms / 1000.0)
    }
}

/// A query's I/O shape: the number of parallel GETs in each serial phase and the
/// total bytes the query reads. The phase vector length is the serial depth `K`.
#[derive(Debug, Clone)]
pub struct QuerySpec {
    /// Parallel GET count per serial phase (`M_k`), length = serial depth `K`.
    pub phase_widths: Vec<usize>,
    /// Total bytes read across all GETs (drives the transfer term).
    pub bytes_read: f64,
    /// Bandwidth used for the transfer term.
    pub bandwidth: Bandwidth,
    /// Human-readable label for reports.
    pub label: &'static str,
}

impl QuerySpec {
    /// The headline in-envelope query: a 6-hop unanchored property-filtered
    /// `MATCH ... LIMIT 10` at the design point.
    ///
    /// * Phase 1 (manifest) and phase 2 (index probe) are single GETs (`M=1`).
    /// * The 6 hop phases each fan out to `M_max` (= 8) parallel range-GETs —
    ///   the frontier-width cap. This is the *worst-case in-envelope* width per
    ///   phase (ADR §2.3: LIMIT-driven early termination keeps it `≤ M_max`).
    /// * `bytes_read` is set to `B_max` for the bandwidth (the worst-case
    ///   in-envelope byte budget), so the simulated query sits at the envelope
    ///   boundary — the hardest in-envelope case.
    #[must_use]
    pub fn headline_in_envelope(bw: Bandwidth) -> Self {
        let params = EnvelopeParams::design_point();
        // 8 phases: manifest(1), index(1), then 6 hops at M_max.
        let mut phase_widths = vec![1usize, 1usize];
        for _ in 0..6 {
            phase_widths.push(params.m_max);
        }
        debug_assert_eq!(phase_widths.len(), params.k_min);
        // Worst-case in-envelope bytes: exactly B_max at the 1 s target.
        let bytes_read = params.b_max_bytes(bw, params.t_target_ms);
        Self {
            phase_widths,
            bytes_read,
            bandwidth: bw,
            label: "headline-in-envelope",
        }
    }

    /// An out-of-envelope query: the same phase structure but a seed set / byte
    /// volume far beyond `B_max` (a low-selectivity filter the planner's
    /// out-of-envelope detection must reject). Demonstrates the sim does not
    /// trivially always pass (AC4).
    #[must_use]
    pub fn out_of_envelope(bw: Bandwidth) -> Self {
        let params = EnvelopeParams::design_point();
        let mut phase_widths = vec![1usize, 1usize];
        for _ in 0..6 {
            phase_widths.push(params.m_max);
        }
        // 50x the byte budget — an unselective filter blowing past B_max.
        let bytes_read = params.b_max_bytes(bw, params.t_ceiling_ms) * 50.0;
        Self {
            phase_widths,
            bytes_read,
            bandwidth: bw,
            label: "out-of-envelope",
        }
    }

    /// A uniform-width latency-only probe: `k` serial phases, each issuing `m`
    /// parallel GETs, with **zero** transfer bytes. This isolates the
    /// `Σ max-of-M` latency term so it can be cross-checked against the
    /// decision-0005 / ADR §1.7 max-of-M amplification table. Bandwidth is
    /// irrelevant (no bytes) so 1 Gbps is used as a placeholder.
    #[must_use]
    pub fn latency_probe(k: usize, m: usize) -> Self {
        Self {
            phase_widths: vec![m; k],
            bytes_read: 0.0,
            bandwidth: Bandwidth::gbps_1(),
            label: "latency-probe",
        }
    }

    /// Serial phase depth `K` of this query.
    #[must_use]
    pub fn k(&self) -> usize {
        self.phase_widths.len()
    }
}

/// The result of a simulation run: end-to-end percentiles plus the decomposed
/// cost-model terms required by the acceptance criteria (the serial floor and
/// the max-of-M latency term must both be visible).
#[derive(Debug, Clone)]
pub struct SimReport {
    /// Query label.
    pub label: String,
    /// Bandwidth label.
    pub bandwidth: &'static str,
    /// Number of trials.
    pub trials: usize,
    /// Bare serial latency floor `K · L_p99` (ms) — the M=1 reference.
    pub serial_floor_ms: f64,
    /// P99 of the *latency-only* term `Σ max-of-M` (ms) — amplified over the
    /// floor by the intra-phase order statistic.
    pub lat_term_p99_ms: f64,
    /// Deterministic byte-transfer term `B/W` (ms).
    pub transfer_ms: f64,
    /// Fixed compute term (ms).
    pub compute_ms: f64,
    /// End-to-end P50 (ms).
    pub total_p50_ms: f64,
    /// End-to-end P95 (ms).
    pub total_p95_ms: f64,
    /// End-to-end P99 (ms) — the headline SLA metric.
    pub total_p99_ms: f64,
    /// End-to-end max observed (ms).
    pub total_max_ms: f64,
    /// Whether the P99 met the 1 s target.
    pub meets_target: bool,
    /// Whether the P99 met the 2 s ceiling.
    pub meets_ceiling: bool,
}

/// Extract the percentile at fractional rank `q ∈ (0, 1]` from a sorted slice,
/// using the position `⌈q · N⌉` (1-indexed) per the cold-start benchmark
/// protocol (ADR-0004 cold-start-benchmark Rule 4).
fn percentile_sorted(sorted: &[f64], q: f64) -> f64 {
    assert!(!sorted.is_empty(), "percentile of empty sample");
    let n = sorted.len();
    let rank = (q * n as f64).ceil() as usize; // 1-indexed
    let idx = rank.clamp(1, n) - 1;
    sorted[idx]
}

/// Run the discrete-event simulation: `trials` independent cold-start queries,
/// each assembling per-phase max-of-M GET latencies + transfer + compute.
///
/// Returns a [`SimReport`] with the end-to-end percentiles and the decomposed
/// terms. Deterministic in `(seed, trials)`.
#[must_use]
pub fn simulate(
    query: &QuerySpec,
    dist: &LatencyDist,
    params: &EnvelopeParams,
    trials: usize,
    seed: u64,
) -> SimReport {
    assert!(trials > 0, "trials must be > 0");
    let mut rng = SplitMix64::new(seed);

    let transfer_ms = query.bytes_read / query.bandwidth.bytes_per_s * 1000.0;
    let compute_ms = params.t_compute_ms;

    let mut totals = Vec::with_capacity(trials);
    let mut lat_terms = Vec::with_capacity(trials);

    for _ in 0..trials {
        // T_lat = Σ over phases of max-of-M_k GET latencies (strictly serial
        // across phases; parallel within a phase).
        let mut t_lat = 0.0_f64;
        for &m in &query.phase_widths {
            let mut phase_max = 0.0_f64;
            for _ in 0..m.max(1) {
                let s = dist.sample_ms(&mut rng);
                if s > phase_max {
                    phase_max = s;
                }
            }
            t_lat += phase_max;
        }
        lat_terms.push(t_lat);
        totals.push(t_lat + transfer_ms + compute_ms);
    }

    totals.sort_by(f64::total_cmp);
    lat_terms.sort_by(f64::total_cmp);

    let total_p99_ms = percentile_sorted(&totals, 0.99);

    SimReport {
        label: query.label.to_string(),
        bandwidth: query.bandwidth.label,
        trials,
        serial_floor_ms: params.serial_floor_ms(),
        lat_term_p99_ms: percentile_sorted(&lat_terms, 0.99),
        transfer_ms,
        compute_ms,
        total_p50_ms: percentile_sorted(&totals, 0.50),
        total_p95_ms: percentile_sorted(&totals, 0.95),
        total_p99_ms,
        total_max_ms: *totals.last().expect("non-empty"),
        meets_target: total_p99_ms <= params.t_target_ms,
        meets_ceiling: total_p99_ms <= params.t_ceiling_ms,
    }
}

/// The **analytical** P99 from ADR-0001 §3.1, for cross-checking the sim:
///
/// ```text
/// T_query(P99) = K · L_p99 · α(M_max) + B_query / W + T_compute
/// ```
///
/// The simulation should agree with this within a stated tolerance; large
/// divergence is a bug in either the model or the sim (formal-verification
/// policy: "discrepancy beyond tolerance is a bug — investigate").
#[must_use]
pub fn analytic_p99_ms(query: &QuerySpec, params: &EnvelopeParams) -> f64 {
    let t_lat = params.serial_floor_ms() * params.alpha;
    let t_transfer = query.bytes_read / query.bandwidth.bytes_per_s * 1000.0;
    t_lat + t_transfer + params.t_compute_ms
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lognormal_fit_recovers_percentiles() {
        let d = LatencyDist::lognormal_from_p50_p99(20.0, 50.0);
        assert!((d.median_ms() - 20.0).abs() < 1e-6);
        assert!((d.p99_ms() - 50.0).abs() < 1e-6);
    }

    #[test]
    fn lognormal_empirical_p99_matches_closed_form() {
        // Sample many draws and confirm the empirical P99 is near the analytic
        // P99 (within 5%), proving the sampler matches the fit.
        let d = LatencyDist::lognormal_from_p50_p99(20.0, 50.0);
        let mut rng = SplitMix64::new(7);
        let mut xs: Vec<f64> = (0..200_000).map(|_| d.sample_ms(&mut rng)).collect();
        xs.sort_by(f64::total_cmp);
        let emp_p99 = percentile_sorted(&xs, 0.99);
        let rel = (emp_p99 - 50.0).abs() / 50.0;
        assert!(rel < 0.05, "empirical p99 {emp_p99} vs 50.0, rel {rel}");
    }

    #[test]
    fn serial_floor_is_k_times_lp99() {
        let p = EnvelopeParams::design_point();
        assert!((p.serial_floor_ms() - 400.0).abs() < 1e-9);
    }

    #[test]
    fn b_max_matches_adr_design_point() {
        let p = EnvelopeParams::design_point();
        // ADR §1.7: B_max(1 Gbps) = 57.5 MB, B_max(50 Mbps) = 2.88 MB at 1 s.
        let b1 = p.b_max_bytes(Bandwidth::gbps_1(), p.t_target_ms);
        let b50 = p.b_max_bytes(Bandwidth::mbps_50(), p.t_target_ms);
        assert!((b1 - 57_500_000.0).abs() < 1.0, "b1 = {b1}");
        assert!((b50 - 2_875_000.0).abs() < 1.0, "b50 = {b50}");
    }

    #[test]
    fn b_max_zero_when_floor_exhausts_budget() {
        let mut p = EnvelopeParams::design_point();
        p.l_p99_ms = 300.0; // floor 8*300*1.1 = 2640 ms > 1 s budget
        assert_eq!(p.b_max_bytes(Bandwidth::gbps_1(), p.t_target_ms), 0.0);
    }

    #[test]
    fn headline_query_has_k8_structure() {
        let q = QuerySpec::headline_in_envelope(Bandwidth::mbps_50());
        assert_eq!(q.k(), 8);
        assert_eq!(q.phase_widths, vec![1, 1, 8, 8, 8, 8, 8, 8]);
    }

    #[test]
    fn percentile_uses_ceil_rank() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        // ceil(0.99*5) = ceil(4.95) = 5 ⇒ index 4 ⇒ 5.0
        assert_eq!(percentile_sorted(&xs, 0.99), 5.0);
        // ceil(0.5*5) = ceil(2.5) = 3 ⇒ index 2 ⇒ 3.0
        assert_eq!(percentile_sorted(&xs, 0.50), 3.0);
    }

    #[test]
    fn analytic_design_point_closes_at_one_second_50mbps() {
        let p = EnvelopeParams::design_point();
        let q = QuerySpec::headline_in_envelope(Bandwidth::mbps_50());
        let a = analytic_p99_ms(&q, &p);
        // T_lat 440 + transfer 460 + compute 100 = 1000 ms by construction.
        assert!((a - 1000.0).abs() < 1.0, "analytic = {a}");
    }

    #[test]
    fn splitmix_is_deterministic() {
        let mut a = SplitMix64::new(123);
        let mut b = SplitMix64::new(123);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn next_f64_in_unit_interval() {
        let mut rng = SplitMix64::new(999);
        for _ in 0..10_000 {
            let x = rng.next_f64();
            assert!((0.0..1.0).contains(&x), "x = {x}");
        }
    }
}
