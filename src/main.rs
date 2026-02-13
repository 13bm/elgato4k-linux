//! Elgato 4K X/S Controller — USB control tool for Linux.
//!
//! A command-line utility for changing settings on the Elgato 4K X (UVC) and
//! 4K S (HID) capture cards.  Run `elgato4k --help` for usage information.

use elgato4k_linux::*;

fn print_usage() {
    println!("Elgato 4K X/S Controller - USB Control Tool\n");
    println!("USAGE:");
    println!("    sudo elgato4k [OPTIONS]\n");
    println!("OPTIONS:");
    println!("    --status                    Read current device settings");
    println!("    --firmware-version          Read firmware version\n");
    println!("    --hdmi-range <VALUE>        Set HDMI color range (EDID range policy)");
    println!("    --edid-range <VALUE>        Alias for --hdmi-range");
    println!("                                Values: expand, shrink, auto");
    println!("                                (expand = Full, shrink = Limited)\n");
    println!("    --edid-source <VALUE>       Set EDID source selection");
    println!("                                Values: display, merged, internal");
    println!("                                  display  = passthrough monitor's EDID");
    println!("                                  merged   = combined EDID from all displays");
    println!("                                  internal = capture card's built-in EDID\n");
    println!("    --hdr-map <VALUE>           Set HDR tone mapping");
    println!("                                Values: on, off\n");
    println!("    --custom-edid <VALUE>       Set custom EDID preset (4K X only)");
    println!("                                Values: on, off");
    println!("                                Note: selects preset index, not file upload\n");
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
    println!("    sudo elgato4k --status");
    println!("    sudo elgato4k --firmware-version");
    println!("    sudo elgato4k --hdr-map on");
    println!("    sudo elgato4k --hdmi-range expand --hdr-map on");
    println!("    sudo elgato4k --edid-source display --hdmi-range auto");
    println!("    sudo elgato4k --custom-edid on");
    println!("    sudo elgato4k --audio-input analog  # 4K S only");
    println!("    sudo elgato4k --video-scaler on     # 4K S only");
    println!("    sudo elgato4k --usb-speed 10g");
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
        let url = "https://api.github.com/repos/bmarr/elgato4k-linux/releases/latest";

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
            println!("   https://github.com/bmarr/elgato4k-linux/releases/latest");
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

    let mut i = 1;
    let mut settings_applied = false;

    while i < args.len() {
        let arg = &args[i];

        if i + 1 >= args.len() {
            return Err(ElgatoError::MissingArgumentValue(arg.clone()).into());
        }

        let value = &args[i + 1];

        match arg.as_str() {
            "--hdmi-range" | "--edid-range" => {
                let range: EdidRangePolicy = value.parse().map_err(|_| ElgatoError::InvalidArgument {
                    arg: "--hdmi-range",
                    value: value.clone(),
                    valid: EdidRangePolicy::VALID_VALUES,
                })?;
                println!("Setting HDMI color range to {}", range);
                device.set_hdmi_range(range)?;
                settings_applied = true;
            }
            "--edid-source" => {
                let source: EdidSource = value.parse().map_err(|_| ElgatoError::InvalidArgument {
                    arg: "--edid-source",
                    value: value.clone(),
                    valid: EdidSource::VALID_VALUES,
                })?;
                println!("Setting EDID source to {}", source);
                device.set_edid_source(source)?;
                settings_applied = true;
            }
            "--hdr-map" => {
                let mode: HdrToneMapping = value.parse().map_err(|_| ElgatoError::InvalidArgument {
                    arg: "--hdr-map",
                    value: value.clone(),
                    valid: HdrToneMapping::VALID_VALUES,
                })?;
                println!("Setting HDR tone mapping to {}", mode);
                device.set_hdr_mapping(mode)?;
                settings_applied = true;
            }
            "--custom-edid" => {
                let mode: CustomEdidMode = value.parse().map_err(|_| ElgatoError::InvalidArgument {
                    arg: "--custom-edid",
                    value: value.clone(),
                    valid: CustomEdidMode::VALID_VALUES,
                })?;
                println!("Setting custom EDID to {}", mode);
                device.set_custom_edid(mode)?;
                settings_applied = true;
            }
            "--audio-input" => {
                let input: AudioInput = value.parse().map_err(|_| ElgatoError::InvalidArgument {
                    arg: "--audio-input",
                    value: value.clone(),
                    valid: AudioInput::VALID_VALUES,
                })?;
                println!("Setting audio input to {}", input);
                device.set_audio_input(input)?;
                settings_applied = true;
            }
            "--video-scaler" => {
                let scaler: VideoScaler = value.parse().map_err(|_| ElgatoError::InvalidArgument {
                    arg: "--video-scaler",
                    value: value.clone(),
                    valid: VideoScaler::VALID_VALUES,
                })?;
                println!("Setting video scaler to {}", scaler);
                device.set_video_scaler(scaler)?;
                settings_applied = true;
            }
            "--usb-speed" => {
                let speed: UsbSpeed = value.parse().map_err(|_| ElgatoError::InvalidArgument {
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
        std::thread::sleep(SETTING_APPLY_DELAY);
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
