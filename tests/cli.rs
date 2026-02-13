//! Integration tests for the `elgato4k-linux` CLI binary.
//!
//! These tests exercise the compiled binary via `std::process::Command`.
//! They do **not** require an Elgato device to be connected — only the
//! help/usage paths can be tested without hardware.

use std::process::Command;

/// Helper: run the binary with the given args.
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_elgato4k-linux"))
        .args(args)
        .output()
        .expect("failed to execute binary")
}

// ── Help / usage ──────────────────────────────────────────────────────

#[test]
fn no_args_shows_usage() {
    let out = run(&[]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE:"), "expected usage text");
    assert!(stdout.contains("--status"), "expected --status in help");
}

#[test]
fn help_flag_shows_usage() {
    let out = run(&["--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE:"));
    assert!(stdout.contains("EXAMPLES:"));
}

#[test]
fn short_help_flag_shows_usage() {
    let out = run(&["-h"]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("USAGE:"));
}

#[test]
fn help_lists_supported_devices() {
    let out = run(&["--help"]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("SUPPORTED DEVICES:"));
    assert!(stdout.contains("Elgato 4K X:"));
    assert!(stdout.contains("Elgato 4K S:"));
}

// ── Error paths (no hardware needed — just verify non-zero exit) ─────

#[test]
fn unknown_flag_exits_nonzero() {
    // NOTE: The CLI opens the USB device before validating most args,
    // so this will fail with "device not found" rather than "unknown option".
    // Either way it must exit non-zero.
    let out = run(&["--bogus-flag", "value"]);
    assert!(!out.status.success());
}

#[test]
fn missing_value_exits_nonzero() {
    // A known flag with no value should error out (device-not-found or
    // missing-argument, depending on arg order vs device open).
    let out = run(&["--hdr-map"]);
    assert!(!out.status.success());
}
