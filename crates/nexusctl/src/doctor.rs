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
