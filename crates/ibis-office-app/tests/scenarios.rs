// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (c) ibis inc.
// Additional terms under AGPL-3.0 section 7(b): see NOTICE.
//! The scenario tests (`src/shell/scenario.rs`): every registered scenario,
//! run in turn on the main thread.
//!
//!   cargo test --release -p ibis-office-app --test scenarios [-- <part of a name> | !<part of a name to leave out>]

/// The scenarios are driven by `src/shell/scenario.rs`, which is macOS-only
/// (the driver draws with the platform's headless renderer, which only macOS
/// has; see `headless_export` in build.rs), so there is nothing to run here.
#[cfg(not(all(target_os = "macos", feature = "scenarios")))]
fn main() {
    eprintln!("the scenario tests run on macOS only");
}

#[cfg(all(target_os = "macos", feature = "scenarios"))]
fn main() {
    let filter: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with('-')).collect();
    std::process::exit(ibis_office_app::run_scenarios(&filter));
}
