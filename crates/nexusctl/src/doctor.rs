//! `nexusctl doctor`: prints the checks from `nexus-doctor`.

use std::process::ExitCode;

use nexus_doctor::State;

pub fn run() -> ExitCode {
    say!("nexusctl {}\n", env!("CARGO_PKG_VERSION"));
    let checks = nexus_doctor::run();
    say!("{}", nexus_doctor::text(&checks));
    if checks.iter().any(|c| c.state == State::Fail) {
        say!("\nSomething needs fixing; see FAIL above.\n");
        ExitCode::FAILURE
    } else {
        say!("\nEverything looks right.\n");
        ExitCode::SUCCESS
    }
}

/// `nexusctl devices`.
pub fn devices() -> ExitCode {
    let list = nexus_doctor::devices();
    if list.is_empty() {
        say!("No Corsair devices found.\n");
        return ExitCode::SUCCESS;
    }
    let width = list
        .iter()
        .map(|d| d.name.chars().count())
        .max()
        .unwrap_or(0);
    for d in &list {
        say!(
            "{:width$}  {:04x}  {}  ({})\n",
            d.name,
            d.id,
            d.state,
            d.support
        );
        for line in d.fix.lines() {
            say!("{:width$}        {line}\n", "");
        }
    }
    ExitCode::SUCCESS
}
