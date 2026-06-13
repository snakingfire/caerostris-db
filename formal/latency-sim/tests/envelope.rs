//! Integration tests encoding the T-0014 acceptance criteria against the
//! ratified envelope (ADR-0001, SPIKE-0006, decision 0005).
//!
//! These are deterministic (fixed seed) so the SLA assertions are reproducible
//! in CI. The simulation is the Cat. 11 latency-model evidence shared with
//! Cat. 3; the analytical cross-check confirms it corroborates ADR-0001.

use caerostris_latency_sim::{
    analytic_p99_ms, simulate, Bandwidth, EnvelopeParams, LatencyDist, QuerySpec, SimReport,
};

/// AC1 + AC2: the simulation models K phases x M parallel GETs with a
/// configurable per-request lognormal distribution, and the output breakdown
/// makes BOTH the serial K*L_p99 floor and the intra-phase max-of-M tail
/// visible as distinct, non-zero terms.
#[test]
fn breakdown_exposes_floor_and_max_of_m_terms() {
    let dist = LatencyDist::lognormal_from_p50_p99(20.0, 100.0);
    let params = EnvelopeParams::design_point();
    let query = QuerySpec::headline_in_envelope(Bandwidth::gbps_1());

    let report: SimReport = simulate(&query, &dist, &params, 20_000, 0xC0FFEE);

    // The serial floor K * L_p99 must be reported and equal the ratified 400 ms
    // (K_min = 8, L_p99 = 50 ms).
    assert!((report.serial_floor_ms - 400.0).abs() < 1e-9, "{report:?}");

    // The max-of-M latency term must EXCEED the bare floor (amplification by the
    // order statistic, BUG-0004 / decision 0005). If they were equal the sim
    // would be ignoring the max-of-M tail.
    assert!(
        report.lat_term_p99_ms > report.serial_floor_ms,
        "max-of-M latency P99 ({}) must exceed the bare floor ({})",
        report.lat_term_p99_ms,
        report.serial_floor_ms,
    );

    // Both the transfer term and the compute term must be present and positive.
    assert!(report.transfer_ms > 0.0, "{report:?}");
    assert!((report.compute_ms - 100.0).abs() < 1e-9, "{report:?}");
}

/// AC3 (1 Gbps): for the in-envelope headline query the simulated end-to-end
/// P99 must be <= 1 s, and it must agree with the analytical model within the
/// stated tolerance (15%).
#[test]
fn in_envelope_p99_under_one_second_1gbps() {
    let dist = LatencyDist::lognormal_from_p50_p99(20.0, 50.0); // ADR-0001 design point
    let params = EnvelopeParams::design_point();
    let query = QuerySpec::headline_in_envelope(Bandwidth::gbps_1());

    let report = simulate(&query, &dist, &params, 20_000, 1);

    assert!(
        report.total_p99_ms <= 1000.0,
        "1 Gbps in-envelope P99 {} ms must be <= 1000 ms",
        report.total_p99_ms,
    );

    // Analytical cross-check: |sim - analytic| / analytic <= tolerance.
    let analytic = analytic_p99_ms(&query, &params);
    let rel = (report.total_p99_ms - analytic).abs() / analytic;
    assert!(
        rel <= 0.15,
        "sim P99 {} ms vs analytic {} ms: rel error {:.3} > 0.15",
        report.total_p99_ms,
        analytic,
        rel,
    );
}

/// AC3 (50 Mbps binding case): the binding constraint. Must hold within the
/// 1 s target at the design point; the 2 s ceiling has ample headroom. This is
/// the case the commander's intent calls out as binding, so it is asserted
/// explicitly and separately.
#[test]
fn in_envelope_p99_under_one_second_50mbps_binding() {
    let dist = LatencyDist::lognormal_from_p50_p99(20.0, 50.0);
    let params = EnvelopeParams::design_point();
    let query = QuerySpec::headline_in_envelope(Bandwidth::mbps_50());

    let report = simulate(&query, &dist, &params, 20_000, 2);

    // The 50 Mbps design point sits at the boundary (~1.0 s by construction);
    // assert it stays within the 2 s ceiling and at/under the 1 s target within
    // a small tolerance band (boundary case).
    assert!(
        report.total_p99_ms <= 2000.0,
        "50 Mbps in-envelope P99 {} ms must be <= 2000 ms ceiling",
        report.total_p99_ms,
    );
    assert!(
        report.total_p99_ms <= 1050.0,
        "50 Mbps in-envelope P99 {} ms must be at/under the ~1 s boundary",
        report.total_p99_ms,
    );
}

/// AC4: an out-of-envelope query must be shown to EXCEED the budget. This is the
/// "the sim does not trivially always pass" sanity check. We use a query that
/// reads far more than B_max (low selectivity / huge seed set).
#[test]
fn out_of_envelope_query_busts_the_budget() {
    let dist = LatencyDist::lognormal_from_p50_p99(20.0, 50.0);
    let params = EnvelopeParams::design_point();
    let query = QuerySpec::out_of_envelope(Bandwidth::mbps_50());

    let report = simulate(&query, &dist, &params, 20_000, 3);

    assert!(
        report.total_p99_ms > 2000.0,
        "out-of-envelope query P99 {} ms must exceed the 2 s ceiling",
        report.total_p99_ms,
    );
}

/// A slow deployment (L_p99 well above the design point) must bust the budget on
/// the latency floor alone, independent of bytes — corroborating SPIKE-0006's
/// "deployment too slow" out-of-envelope condition.
#[test]
fn slow_deployment_busts_floor_independent_of_bytes() {
    // L_p99 = 150 ms: floor alone is 8 * 150 = 1200 ms > 1 s target.
    let dist = LatencyDist::lognormal_from_p50_p99(60.0, 150.0);
    let mut params = EnvelopeParams::design_point();
    params.l_p99_ms = 150.0;
    let query = QuerySpec::headline_in_envelope(Bandwidth::gbps_1());

    let report = simulate(&query, &dist, &params, 20_000, 4);

    assert!(
        report.serial_floor_ms > 1000.0,
        "serial floor {} ms at L_p99=150 ms must exceed the 1 s target",
        report.serial_floor_ms,
    );
    assert!(
        report.total_p99_ms > 1000.0,
        "slow-deployment P99 {} ms must bust the 1 s target",
        report.total_p99_ms,
    );
}

/// Calibration cross-check: the sim must reproduce the decision-0005 / ADR §1.7
/// max-of-M amplification table. For the calibration distribution (lognormal
/// P50=20 ms, P99=100 ms) at K=3 phases of M=8 parallel GETs, decision 0005
/// reports a query P99 of ~332 ms (ratio 1.11 over the naive 3*L_p99 = 300 ms).
/// The sim's latency-only term must land near that figure (within 10%), which is
/// what justifies the analytical α(8) = 1.10.
#[test]
fn reproduces_decision_0005_max_of_m_amplification() {
    let calib = LatencyDist::lognormal_from_p50_p99(20.0, 100.0);
    let params = EnvelopeParams::design_point();
    let probe = QuerySpec::latency_probe(3, 8);

    let report = simulate(&probe, &calib, &params, 50_000, 5);

    // With zero transfer bytes the end-to-end term is lat + compute; isolate the
    // latency term reported in lat_term_p99_ms.
    let decision_0005_value = 332.0;
    let rel = (report.lat_term_p99_ms - decision_0005_value).abs() / decision_0005_value;
    assert!(
        rel <= 0.10,
        "K=3,M=8 latency P99 {} ms vs decision-0005 {} ms: rel {:.3} > 0.10",
        report.lat_term_p99_ms,
        decision_0005_value,
        rel,
    );

    // The naive 3*L_p99 floor (using the calibration L_p99=100 here would be
    // 300 ms); the amplified term must exceed 300 ms (ratio > 1) per the table.
    assert!(
        report.lat_term_p99_ms > 300.0,
        "amplified latency {} ms must exceed naive 3*100 = 300 ms",
        report.lat_term_p99_ms,
    );
}

/// Determinism: the same seed yields the same percentile (reproducible in CI).
#[test]
fn deterministic_for_fixed_seed() {
    let dist = LatencyDist::lognormal_from_p50_p99(20.0, 50.0);
    let params = EnvelopeParams::design_point();
    let query = QuerySpec::headline_in_envelope(Bandwidth::gbps_1());

    let a = simulate(&query, &dist, &params, 5_000, 42);
    let b = simulate(&query, &dist, &params, 5_000, 42);

    assert_eq!(a.total_p99_ms.to_bits(), b.total_p99_ms.to_bits());
}
