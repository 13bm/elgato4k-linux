//! Elgato 4K X/S Controller — USB control tool for Linux.
//!
//! A command-line utility for changing settings on the Elgato 4K X (UVC) and
//! 4K S (HID) capture cards.  Run `elgato4k --help` for usage information.

use std::fmt;

use elgato4k_linux::*;

/// Delay between consecutive setting changes to give the device time to process.
const SETTING_APPLY_DELAY: std::time::Duration = std::time::Duration::from_millis(100);

/// CLI-specific errors for argument parsing.
#[derive(Debug)]
enum CliError {
    /// Invalid CLI argument value.
    InvalidArgument {
        arg: &'static str,
        value: String,
        valid: &'static str,
    },
    /// A required CLI argument value is missing.
    MissingArgumentValue(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument { arg, value, valid } => {
                write!(f, "Invalid value '{}' for {}.\nValid values: {}", value, arg, valid)
            }
            Self::MissingArgumentValue(arg) => {
                write!(f, "{} requires a value", arg)
            }
        }
    }
}

impl std::error::Error for CliError {}

fn print_usage() {
    println!("Elgato 4K X/S Controller - USB Control Tool\n");
    println!("USAGE:");
    println!("    sudo elgato4k-linux [OPTIONS]\n");
    println!("OPTIONS:");
    println!("    --status                    Read current device settings");
    println!("    --signal                    Read HDMI input resolution/rate and lock (4K X)");
    println!("                                Uses bound V4L2 driver; capture stays active");
    println!("    --video-device <PATH>       Select video node for --signal (auto by default)");
    println!("                                Requires variable-length XU driver support\n");
    println!("                                Other USB control options interrupt capture\n");
    println!("    --firmware-version          Read firmware version\n");
    println!("    --dump-edid-table <PATH>    Read all seven 4K X EDID slots to a file\n");
    println!("    --upload-edid <PATH>         Upload an EDID (requires --slot and confirmation)");
    println!("    --slot <0-6>                 EDID flash slot for upload");
    println!("    --source-table <PATH>        Use a saved 0xE00 table, then enter rescue");
    println!("    --resume-table <PATH>        Resume from a saved 0xE00 table after rescue");
    println!("    --confirm-edid-write         Confirm the destructive EDID flash operation\n");
    println!("    --finalize-edid-update       Exit an interrupted, completed rescue update");
    println!("    --hdmi-range <VALUE>        Set HDMI color range");
    println!("                                Values: auto, expand, shrink");
    println!("                                  auto   = match input source (recommended)");
    println!("                                  expand = limited (16-235) to full (0-255)");
    println!("                                  shrink = full (0-255) to limited (16-235)\n");
    println!("    --edid-source <VALUE>       Set EDID source selection");
    println!("                                Values: display, merged, internal");
    println!("                                  display  = passthrough monitor's EDID");
    println!("                                  merged   = combined EDID from all displays");
    println!("                                  internal = capture card's built-in EDID\n");
    println!("    --hdmi-hpd <on|off>         Set HDMI receiver hot-plug state\n");
    println!("    --hdr-map <VALUE>           Set HDR tone mapping");
    println!("                                Values: on, off\n");
    println!("    --audio-input <VALUE>        Set audio input source (4K S only)");
    println!("                                Values: embedded, analog");
    println!("                                (embedded = HDMI audio, analog = line-in)\n");
    println!("    --video-scaler <VALUE>      Enable/disable video scaler (4K S only)");
    println!("                                Values: on, off\n");
    println!("    --usb-speed <VALUE>         Set USB speed mode (4K X only)");
    println!("                                Values: 5g, 10g");
    println!("                                WARNING: Device will disconnect and");
    println!("                                re-enumerate with a different PID\n");
    println!("    --help, -h                  Show this help message\n");
    println!("EXAMPLES:");
    println!("    sudo elgato4k-linux --status");
    println!("    sudo elgato4k-linux --firmware-version");
    println!("    sudo elgato4k-linux --hdr-map on");
    println!("    sudo elgato4k-linux --hdmi-range expand --hdr-map on");
    println!("    sudo elgato4k-linux --edid-source display --hdmi-range auto");
    println!("    sudo elgato4k-linux --audio-input analog  # 4K S only");
    println!("    sudo elgato4k-linux --video-scaler on     # 4K S only");
    println!("    sudo elgato4k-linux --usb-speed 10g");
    println!("\nSUPPORTED DEVICES:");
    println!("    Elgato 4K X:");
    println!("      0fd9:009b  (10Gbps / SuperSpeed+)");
    println!("      0fd9:009c  (5Gbps / SuperSpeed)");
    println!("      0fd9:009d  (USB 2.0)");
    println!("    Elgato 4K S:");
    println!("      0fd9:00af  (USB 3.0)");
    println!("      0fd9:00ae  (USB 2.0)");
}

/// Check GitHub for a newer release. Returns silently on any failure.
fn check_for_update() {
    #[cfg(not(feature = "update-check"))]
    return;

    #[cfg(feature = "update-check")]
    {
        let current = env!("CARGO_PKG_VERSION");
        let url = "https://api.github.com/repos/13bm/elgato4k-linux/releases/latest";

        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(3)))
            .build()
            .into();

        if let Some(latest) = agent.get(url)
            .header("User-Agent", "elgato4k-linux")
            .header("Accept", "application/vnd.github.v3+json")
            .call()
            .and_then(|resp| resp.into_body().read_to_string())
            .ok()
            .and_then(|body| extract_tag_name(&body))
            .filter(|v| is_newer(v, current))
        {
            println!("\nUpdate available: v{} -> v{}", current, latest);
            println!("   https://github.com/13bm/elgato4k-linux/releases/latest");
        }
    }
}

/// Extract version from `"tag_name":"vX.Y.Z"` in a JSON response body.
fn extract_tag_name(json: &str) -> Option<String> {
    let marker = "\"tag_name\":\"";
    let start = json.find(marker)? + marker.len();
    let end = json[start..].find('"')? + start;
    let tag = &json[start..end];
    Some(tag.strip_prefix('v').unwrap_or(tag).to_string())
}

/// Compare semver strings: is `latest` newer than `current`?
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let l = parse(latest);
    let c = parse(current);
    l > c
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return Ok(());
    }

    // Handle live timing before opening libusb, which detaches uvcvideo.
    if args.iter().any(|arg| arg == "--signal") {
        let mut video_device = None;
        let mut index = 1;
        while index < args.len() {
            match args[index].as_str() {
                "--signal" => index += 1,
                "--video-device" if video_device.is_none() => {
                    let path = args
                        .get(index + 1)
                        .filter(|path| !path.starts_with('-'))
                        .ok_or_else(|| CliError::MissingArgumentValue("--video-device".into()))?;
                    video_device = Some(std::path::PathBuf::from(path));
                    index += 2;
                }
                _ => {
                    return Err(
                        "--signal accepts only --video-device; run other controls separately"
                            .into(),
                    );
                }
            }
        }
        let path = match video_device {
            Some(path) => path,
            None => find_hdmi_video_device()?,
        };
        println!("HDMI input: {}", read_hdmi_signal_v4l2(path)?);
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--video-device") {
        return Err("--video-device requires --signal".into());
    }

    if args.iter().any(|arg| arg == "--finalize-edid-update") {
        if args[1..].iter().any(|arg| {
            !matches!(arg.as_str(), "--finalize-edid-update" | "--confirm-edid-write")
        }) {
            return Err("--finalize-edid-update accepts only --confirm-edid-write".into());
        }
        if !args.iter().any(|arg| arg == "--confirm-edid-write") {
            return Err("EDID finalization requires --confirm-edid-write".into());
        }
        ElgatoDevice::finalize_pending_edid_update()?;
        println!("EDID rescue update finalized; normal device re-enumerated");
        return Ok(());
    }

    if let Some(index) = args.iter().position(|arg| arg == "--upload-edid") {
        let mut index_check = 1;
        while index_check < args.len() {
            match args[index_check].as_str() {
                "--confirm-edid-write" => index_check += 1,
                "--upload-edid" | "--slot" | "--source-table" | "--resume-table" => {
                    args.get(index_check + 1)
                        .filter(|value| !value.starts_with('-'))
                        .ok_or_else(|| CliError::MissingArgumentValue(args[index_check].clone()))?;
                    index_check += 2;
                }
                _ => return Err("run EDID upload separately from other controls".into()),
            }
        }
        if args.iter().any(|arg| arg == "--source-table")
            && args.iter().any(|arg| arg == "--resume-table")
        {
            return Err("--source-table and --resume-table cannot be combined".into());
        }
        let path = args
            .get(index + 1)
            .ok_or_else(|| CliError::MissingArgumentValue("--upload-edid".into()))?;
        if !args.iter().any(|arg| arg == "--confirm-edid-write") {
            return Err("EDID upload requires --confirm-edid-write".into());
        }
        let slot_index = args
            .iter()
            .position(|arg| arg == "--slot")
            .ok_or("EDID upload requires --slot 0-6")?;
        let slot: u8 = args
            .get(slot_index + 1)
            .ok_or_else(|| CliError::MissingArgumentValue("--slot".into()))?
            .parse()
            .map_err(|_| "EDID slot must be an integer from 0 through 6")?;
        if slot > 6 {
            return Err("EDID slot must be an integer from 0 through 6".into());
        }
        let edid = std::fs::read(path)?;
        validate_edid(&edid)?;
        println!("Uploading {} bytes to EDID slot {}", edid.len(), slot);
        if let Some(source_index) = args.iter().position(|arg| arg == "--source-table") {
            let table_path = args
                .get(source_index + 1)
                .ok_or_else(|| CliError::MissingArgumentValue("--source-table".into()))?;
            let table = std::fs::read(table_path)?;
            EdidUploadPlan::build(&table, slot, &edid)?;
            ElgatoDevice::open()?.upload_edid_from_table(&table, slot, &edid)?;
        } else if let Some(resume_index) = args.iter().position(|arg| arg == "--resume-table") {
            let table_path = args
                .get(resume_index + 1)
                .ok_or_else(|| CliError::MissingArgumentValue("--resume-table".into()))?;
            let table = std::fs::read(table_path)?;
            ElgatoDevice::resume_edid_upload(&table, slot, &edid)?;
        } else {
            ElgatoDevice::open()?.upload_edid(slot, &edid)?;
        }
        println!("EDID upload and complete-table readback verification succeeded");
        return Ok(());
    }

    for option in ["--slot", "--source-table", "--resume-table", "--confirm-edid-write"] {
        if args.iter().any(|arg| arg == option) {
            return Err(format!("{option} requires --upload-edid").into());
        }
    }

    let device = ElgatoDevice::open()?;

    // Handle flags that don't require a value
    if args.iter().any(|a| a == "--status") {
        println!("Reading current settings from {} (PID: 0x{:04x})...\n", device.model(), device.pid());
        print!("{}", device.read_status()?);
        return Ok(());
    }

    if args.iter().any(|a| a == "--firmware-version") {
        println!("Firmware version: {}", device.read_firmware_version()?);
        return Ok(());
    }

    if let Some(index) = args.iter().position(|arg| arg == "--dump-edid-table") {
        let path = args
            .get(index + 1)
            .ok_or_else(|| CliError::MissingArgumentValue("--dump-edid-table".into()))?;
        let table = device.read_edid_table()?;
        std::fs::write(path, &table)?;
        println!("Read {} bytes to {}", table.len(), path);
        return Ok(());
    }

    let mut i = 1;
    let mut settings_applied = false;

    while i < args.len() {
        let arg = &args[i];

        if i + 1 >= args.len() {
            return Err(CliError::MissingArgumentValue(arg.clone()).into());
        }

        let value = &args[i + 1];

        match arg.as_str() {
            "--hdmi-range" => {
                let range: EdidRangePolicy = value.parse().map_err(|_| CliError::InvalidArgument {
                    arg: "--hdmi-range",
                    value: value.clone(),
                    valid: EdidRangePolicy::VALID_VALUES,
                })?;
                println!("Setting HDMI color range to {}", range);
                device.set_hdmi_range(range)?;
                settings_applied = true;
            }
            "--edid-source" => {
                let source: EdidSource = value.parse().map_err(|_| CliError::InvalidArgument {
                    arg: "--edid-source",
                    value: value.clone(),
                    valid: EdidSource::VALID_VALUES,
                })?;
                println!("Setting EDID source to {}", source);
                device.set_edid_source(source)?;
                settings_applied = true;
            }
            "--hdmi-hpd" => {
                let enabled = match value.as_str() {
                    "on" | "true" | "1" => true,
                    "off" | "false" | "0" => false,
                    _ => return Err("HDMI HPD must be on or off".into()),
                };
                println!(
                    "Setting HDMI hot-plug to {}",
                    if enabled { "On" } else { "Off" }
                );
                device.set_hdmi_hpd(enabled)?;
                settings_applied = true;
            }
            "--hdr-map" => {
                let mode: HdrToneMapping = value.parse().map_err(|_| CliError::InvalidArgument {
                    arg: "--hdr-map",
                    value: value.clone(),
                    valid: HdrToneMapping::VALID_VALUES,
                })?;
                println!("Setting HDR tone mapping to {}", mode);
                device.set_hdr_mapping(mode)?;
                settings_applied = true;
            }
            "--audio-input" => {
                let input: AudioInput = value.parse().map_err(|_| CliError::InvalidArgument {
                    arg: "--audio-input",
                    value: value.clone(),
                    valid: AudioInput::VALID_VALUES,
                })?;
                println!("Setting audio input to {}", input);
                device.set_audio_input(input)?;
                settings_applied = true;
            }
            "--video-scaler" => {
                let scaler: VideoScaler = value.parse().map_err(|_| CliError::InvalidArgument {
                    arg: "--video-scaler",
                    value: value.clone(),
                    valid: VideoScaler::VALID_VALUES,
                })?;
                println!("Setting video scaler to {}", scaler);
                device.set_video_scaler(scaler)?;
                settings_applied = true;
            }
            "--usb-speed" => {
                let speed: UsbSpeed = value.parse().map_err(|_| CliError::InvalidArgument {
                    arg: "--usb-speed",
                    value: value.clone(),
                    valid: UsbSpeed::VALID_VALUES,
                })?;
                println!("Setting USB speed to {}", speed);
                println!("WARNING: Device will disconnect and re-enumerate with a different PID!");
                device.set_usb_speed(speed)?;
                settings_applied = true;
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", arg);
                print_usage();
                return Err("Unknown option".into());
            }
        }

        i += 2;
        // Delay between consecutive settings, but not after the last one
        if i < args.len() {
            std::thread::sleep(SETTING_APPLY_DELAY);
        }
    }

    if settings_applied {
        println!("\nAll settings applied successfully!");
    } else {
        println!("No settings were changed.");
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let result = run();
    check_for_update();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_tag_with_v_prefix() {
        let json = r#"{"tag_name":"v0.3.0","name":"v0.3.0"}"#;
        assert_eq!(extract_tag_name(json), Some("0.3.0".to_string()));
    }

    #[test]
    fn extract_tag_without_v_prefix() {
        let json = r#"{"tag_name":"0.3.0","name":"0.3.0"}"#;
        assert_eq!(extract_tag_name(json), Some("0.3.0".to_string()));
    }

    #[test]
    fn extract_tag_missing() {
        let json = r#"{"name":"v0.3.0"}"#;
        assert_eq!(extract_tag_name(json), None);
    }

    #[test]
    fn newer_version() {
        assert!(is_newer("0.3.0", "0.2.0"));
        assert!(is_newer("0.2.1", "0.2.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn same_version() {
        assert!(!is_newer("0.2.0", "0.2.0"));
    }

    #[test]
    fn older_version() {
        assert!(!is_newer("0.1.0", "0.2.0"));
    }
}
