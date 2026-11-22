use std::process::Command;

#[test]
fn binary_reports_version_and_help() {
    let exe = env!("CARGO_BIN_EXE_bitter-cli");
    let version = Command::new(exe).arg("--version").output().unwrap();
    assert!(version.status.success());
    assert!(String::from_utf8_lossy(&version.stdout).contains("Bitter Browser"));

    let help = Command::new(exe).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("Usage:"));

    let bad = Command::new(exe).arg("--bogus").output().unwrap();
    assert_eq!(bad.status.code(), Some(2));
}
