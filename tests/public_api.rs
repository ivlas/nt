use std::process::ExitCode;

#[test]
fn process_entry_point_remains_public() {
    let _run_process: fn() -> ExitCode = nt::run_process;
}
