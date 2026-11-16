//! `nexusctl doctor`: prints the checks from `nexus-doctor`.

use std::process::ExitCode;

use nexus_doctor::State;

pub fn run() -> ExitCode {
    println!("nexusctl {}", env!("CARGO_PKG_VERSION"));
    let checks = nexus_doctor::run();
    print!("{}", nexus_doctor::text(&checks));
    if checks.iter().any(|c| c.state == State::Fail) {
        println!("\nSomething needs fixing; see FAIL above.");
        ExitCode::FAILURE
    } else {
        println!("\nEverything looks right.");
        ExitCode::SUCCESS
    }
}
