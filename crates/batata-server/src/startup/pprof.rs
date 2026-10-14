//! On-demand CPU flamegraph endpoint (pprof-rs).
//!
//! Gated behind the `pprof` cargo feature. CPU sampling requires Linux and
//! `perf` privileges (CAP_SYS_ADMIN or a lowered `perf_event_paranoid`), the same
//! kernel prerequisite as `perf record`. On non-Linux hosts or without those
//! privileges the handler returns an explanatory error instead of a flamegraph.
//!
//! Usage:
//!   GET /debug/pprof/flamegraph?seconds=30  ->  image/svg+xml (open in a browser)
//!
//! Trigger this while applying load (e.g. `scripts/benchmark.sh`) to see exactly
//! where CPU is spent.

#![cfg(feature = "pprof")]

use actix_web::{HttpResponse, web};
use std::time::Duration;

/// Register the pprof routes on the given actix `ServiceConfig`.
pub fn configure_pprof(cfg: &mut web::ServiceConfig) {
    cfg.route("/debug/pprof/flamegraph", web::get().to(flamegraph_handler));
}

#[derive(serde::Deserialize)]
struct FlamegraphQuery {
    #[serde(default = "default_seconds")]
    seconds: u32,
}

fn default_seconds() -> u32 {
    30
}

async fn flamegraph_handler(query: web::Query<FlamegraphQuery>) -> HttpResponse {
    let seconds = (query.seconds).clamp(1, 300) as u64;

    // Start a process-wide sampling guard. The `blocklist` avoids signal-safety
    // deadlocks inside `_Unwind_Backtrace` and broken vdso dwarf info.
    let guard = match pprof::ProfilerGuardBuilder::default()
        .frequency(100)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
    {
        Ok(g) => g,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!(
                "failed to start profiler: {e} (CPU sampling needs Linux + perf privileges)"
            ));
        }
    };

    // Sample the whole process (all tokio worker threads) for the requested time.
    tokio::time::sleep(Duration::from_secs(seconds)).await;

    let report = match guard.report().build() {
        Ok(r) => r,
        Err(e) => {
            return HttpResponse::InternalServerError()
                .body(format!("failed to build report: {e}"));
        }
    };

    let mut body = Vec::new();
    match report.flamegraph(&mut body) {
        Ok(()) => HttpResponse::Ok().content_type("image/svg+xml").body(body),
        Err(e) => {
            HttpResponse::InternalServerError().body(format!("failed to render flamegraph: {e}"))
        }
    }
}
