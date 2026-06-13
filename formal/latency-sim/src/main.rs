//! CLI driver for the caerostris-db cold-start latency simulation (T-0014).
//!
//! Runs the canonical validation scenarios against the ratified envelope
//! (ADR-0001) and prints a human-readable report plus a one-line machine
//! verdict. Exit code is non-zero if any in-envelope SLA assertion fails or if
//! the out-of-envelope sanity case does NOT bust the budget — so this binary
//! doubles as a CI check, invoked via `--manifest-path` in the
//! `latency-sim` CI job (`.github/workflows/ci.yml`).
//!
//! Run locally:
//! ```text
//! cargo run --manifest-path formal/latency-sim/Cargo.toml --release
//! ```
//!
//! Optional args: `--trials N` (default 100_000), `--seed S` (default 1).

use std::process::ExitCode;

use caerostris_latency_sim::{
    analytic_p99_ms, simulate, Bandwidth, EnvelopeParams, LatencyDist, QuerySpec, SimReport,
};

fn parse_args() -> (usize, u64) {
    let mut trials = 100_000usize;
    let mut seed = 1u64;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--trials" => {
                if let Some(v) = it.next() {
                    trials = v.parse().unwrap_or(trials);
                }
            }
            "--seed" => {
                if let Some(v) = it.next() {
                    seed = v.parse().unwrap_or(seed);
                }
            }
            _ => {}
        }
    }
    (trials, seed)
}

fn print_report(r: &SimReport, analytic_ms: f64) {
    println!("  query            : {}", r.label);
    println!("  bandwidth        : {}", r.bandwidth);
    println!("  trials           : {}", r.trials);
    println!(
        "  serial floor     : {:>8.1} ms   (K * L_p99, the M=1 reference)",
        r.serial_floor_ms
    );
    println!(
        "  lat term P99     : {:>8.1} ms   (Σ max-of-M; amplified over floor)",
        r.lat_term_p99_ms
    );
    println!(
        "  transfer (B/W)   : {:>8.1} ms   (deterministic)",
        r.transfer_ms
    );
    println!("  compute          : {:>8.1} ms   (fixed)", r.compute_ms);
    println!("  ---");
    println!("  end-to-end P50   : {:>8.1} ms", r.total_p50_ms);
    println!("  end-to-end P95   : {:>8.1} ms", r.total_p95_ms);
    println!(
        "  end-to-end P99   : {:>8.1} ms   <-- SLA metric",
        r.total_p99_ms
    );
    println!("  end-to-end max   : {:>8.1} ms", r.total_max_ms);
    println!("  analytic P99     : {analytic_ms:>8.1} ms   (ADR-0001 §3.1)");
    let rel = (r.total_p99_ms - analytic_ms).abs() / analytic_ms;
    println!(
        "  |sim-analytic|   : {:>7.2}%   (tolerance 15%)",
        rel * 100.0
    );
    println!(
        "  meets 1 s target : {}    meets 2 s ceiling: {}",
        if r.meets_target { "YES" } else { "NO " },
        if r.meets_ceiling { "YES" } else { "NO" },
    );
}

fn main() -> ExitCode {
    let (trials, seed) = parse_args();
    let params = EnvelopeParams::design_point();

    // ADR §3.4 design-point GET distribution: lognormal P50=20 ms, P99=50 ms.
    let dist = LatencyDist::lognormal_from_p50_p99(20.0, 50.0);

    println!("==============================================================");
    println!(" caerostris-db cold-start latency simulation  (T-0014)");
    println!(
        " envelope: ADR-0001  |  K_min={}  L_p99={} ms  M_max={}  α={}",
        params.k_min, params.l_p99_ms, params.m_max, params.alpha
    );
    println!(
        " GET dist: lognormal  P50={:.0} ms  P99={:.0} ms  |  cache: OFF",
        dist.median_ms(),
        dist.p99_ms()
    );
    println!("==============================================================");

    let mut ok = true;

    // Scenario 1: in-envelope @ 1 Gbps — target P99 <= 1 s.
    println!("\n[1] in-envelope headline query @ 1 Gbps  (target: P99 <= 1 s)");
    let q = QuerySpec::headline_in_envelope(Bandwidth::gbps_1());
    let r = simulate(&q, &dist, &params, trials, seed);
    print_report(&r, analytic_p99_ms(&q, &params));
    if !r.meets_target {
        eprintln!("  FAIL: 1 Gbps in-envelope P99 exceeded the 1 s target");
        ok = false;
    }

    // Scenario 2: in-envelope @ 50 Mbps — the BINDING case.
    println!("\n[2] in-envelope headline query @ 50 Mbps  (binding; ceiling: P99 <= 2 s)");
    let q = QuerySpec::headline_in_envelope(Bandwidth::mbps_50());
    let r = simulate(&q, &dist, &params, trials, seed + 1);
    print_report(&r, analytic_p99_ms(&q, &params));
    if !r.meets_ceiling {
        eprintln!("  FAIL: 50 Mbps in-envelope P99 exceeded the 2 s ceiling");
        ok = false;
    }

    // Scenario 3: out-of-envelope @ 50 Mbps — MUST bust the budget (sanity).
    println!("\n[3] out-of-envelope query @ 50 Mbps  (sanity: MUST exceed 2 s ceiling)");
    let q = QuerySpec::out_of_envelope(Bandwidth::mbps_50());
    let r = simulate(&q, &dist, &params, trials, seed + 2);
    print_report(&r, analytic_p99_ms(&q, &params));
    if r.meets_ceiling {
        eprintln!("  FAIL: out-of-envelope query did NOT bust the budget (sim trivially passes)");
        ok = false;
    }

    // Scenario 4: slow deployment (L_p99=150 ms) — floor alone busts 1 s target.
    println!("\n[4] slow deployment L_p99=150 ms @ 1 Gbps  (sanity: floor alone busts 1 s)");
    let slow_dist = LatencyDist::lognormal_from_p50_p99(60.0, 150.0);
    let mut slow_params = params;
    slow_params.l_p99_ms = 150.0;
    let q = QuerySpec::headline_in_envelope(Bandwidth::gbps_1());
    let r = simulate(&q, &slow_dist, &slow_params, trials, seed + 3);
    print_report(&r, analytic_p99_ms(&q, &slow_params));
    if r.meets_target {
        eprintln!("  FAIL: slow deployment did NOT bust the 1 s target");
        ok = false;
    }

    println!("\n==============================================================");
    if ok {
        println!(" VERDICT: PASS — the latency theorem holds in simulation.");
        println!(" In-envelope queries meet the SLA cold, cache OFF; out-of-");
        println!(" envelope and slow-deployment cases correctly bust the budget.");
        println!("==============================================================");
        ExitCode::SUCCESS
    } else {
        println!(" VERDICT: FAIL — see [FAIL] lines above. This is a P0 Cat. 3 gap.");
        println!("==============================================================");
        ExitCode::FAILURE
    }
}
