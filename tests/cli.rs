//! Integration tests for the `elgato4k-linux` CLI binary.
//!
//! These tests exercise the compiled binary via `std::process::Command`.
//! They do **not** require an Elgato device to be connected — only the
//! help/usage and early argument validation paths can be tested without hardware.

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
    assert!(
        stdout.contains("--signal"),
        "expected HDMI signal option in help"
    );
    assert!(stdout.contains("capture stays active"));
    assert!(stdout.contains("--video-device"));
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

#[test]
fn live_signal_does_not_fall_back_to_usb() {
    let out = run(&[
        "--signal",
        "--video-device",
        "/definitely/not/a/video-device",
    ]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("open video device"));
}

#[test]
fn live_signal_rejects_other_actions_before_device_open() {
    for args in [
        vec!["--signal", "--hdr-map", "on"],
        vec!["--signal", "--status"],
        vec!["--signal", "--video-device", "--hdr-map"],
        vec!["--video-device", "/dev/video0"],
    ] {
        let out = run(&args);
        assert!(!out.status.success());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(!error.contains("not found. Make sure"));
    }
}

#[test]
fn live_signal_rejects_edid_recovery_before_updater_access() {
    for args in [
        vec!["--signal", "--finalize-edid-update"],
        vec!["--signal", "--finalize-edid-update", "--confirm-edid-write"],
        vec![
            "--signal",
            "--upload-edid",
            "custom.bin",
            "--confirm-edid-write",
        ],
    ] {
        let out = run(&args);
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("--signal accepts only --video-device")
        );
    }
}

#[test]
fn edid_upload_requires_confirmation_before_usb_access() {
    let out = run(&["--upload-edid", "custom.bin", "--slot", "0"]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("requires --confirm-edid-write"));
}

#[test]
fn edid_actions_reject_conflicting_options_before_usb_access() {
    for args in [
        vec!["--upload-edid", "custom.bin", "--status"],
        vec!["--upload-edid", "--slot", "0"],
        vec![
            "--upload-edid",
            "custom.bin",
            "--source-table",
            "a.bin",
            "--resume-table",
            "b.bin",
        ],
        vec![
            "--finalize-edid-update",
            "--confirm-edid-write",
            "--hdr-map",
            "on",
        ],
        vec!["--resume-table", "table.bin"],
    ] {
        let out = run(&args);
        assert!(!out.status.success());
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(!error.contains("not found. Make sure"));
        assert!(!error.contains("did not appear before timeout"));
    }
}

#[test]
fn edid_resume_validates_table_without_a_normal_usb_handle() {
    let error = elgato4k_linux::ElgatoDevice::resume_edid_upload(&[], 0, &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("Invalid EDID data"));
}
