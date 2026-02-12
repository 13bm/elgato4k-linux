mod device;
mod hid;
mod settings;
mod status;
mod uvc;

use device::ElgatoDevice;
use settings::*;

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        print_usage();
        return Ok(());
    }

    let device = ElgatoDevice::open()?;

    // Handle flags that don't require a value
    if args.contains(&"--status".to_string()) {
        device.get_status()?;
        return Ok(());
    }

    if args.contains(&"--firmware-version".to_string()) {
        device.get_firmware_version()?;
        return Ok(());
    }

    let mut i = 1;
    let mut settings_applied = false;

    while i < args.len() {
        let arg = &args[i];

        if i + 1 >= args.len() {
            eprintln!("Error: {} requires a value", arg);
            return Err("Missing argument value".into());
        }

        let value = &args[i + 1];

        match arg.as_str() {
            "--hdmi-range" | "--edid-range" => {
                if let Some(range) = EdidRangePolicy::from_str(value) {
                    println!("Setting HDMI color range to {:?}", range);
                    match device.model() {
                        DeviceModel::Elgato4KX => device.set_uvc_setting(range.payload_4kx())?,
                        DeviceModel::Elgato4KS => {
                            let (pkt1, pkt2) = range.payload_4ks();
                            device.send_hid_two_packet(pkt1, pkt2)?;
                        }
                    }
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for {}", value, arg);
                    eprintln!("Valid values: expand, shrink, auto");
                    return Err("Invalid argument".into());
                }
            }
            "--edid-source" => {
                if let Some(source) = EdidSource::from_str(value) {
                    println!("Setting EDID source to {:?}", source);
                    match device.model() {
                        DeviceModel::Elgato4KX => device.set_uvc_setting(source.payload_4kx())?,
                        DeviceModel::Elgato4KS => device.send_hid_packet(&source.payload_4ks())?,
                    }
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for --edid-source", value);
                    eprintln!("Valid values: display, merged, internal");
                    return Err("Invalid argument".into());
                }
            }
            "--hdr-map" => {
                if let Some(mode) = HdrToneMapping::from_str(value) {
                    println!("Setting HDR tone mapping to {:?}", mode);
                    match device.model() {
                        DeviceModel::Elgato4KX => device.set_uvc_setting(mode.payload_4kx())?,
                        DeviceModel::Elgato4KS => {
                            let (pkt1, pkt2) = mode.payload_4ks();
                            device.send_hid_two_packet(pkt1, pkt2)?;
                        }
                    }
                    settings_applied = true;
                } else {
                    eprintln!("Error: Invalid value '{}' for --hdr-map", value);
                    eprintln!("Valid values: on, off");
                    return Err("Invalid argument".into());
                }
            }
            "--custom-edid" => {
                if let Some(mode) = CustomEdidMode::from_str(value) {
                    match device.model() {
                        DeviceModel::Elgato4KX => {
                            println!("Setting custom EDID to {:?}", mode);
                            device.set_uvc_setting(mode.payload_4kx())?;
                            settings_applied = true;
                        }
                        DeviceModel::Elgato4KS => {
                            eprintln!("Error: Custom EDID is not supported on 4K S");
                            return Err("Unsupported feature".into());
                        }
                    }
                } else {
                    eprintln!("Error: Invalid value '{}' for --custom-edid", value);
                    eprintln!("Valid values: on, off");
                    return Err("Invalid argument".into());
                }
            }
            "--audio-input" => {
                if let Some(input) = AudioInput::from_str(value) {
                    match device.model() {
                        DeviceModel::Elgato4KS => {
                            println!("Setting audio input to {:?}", input);
                            let (pkt1, pkt2) = input.payload_4ks();
                            device.send_hid_two_packet(pkt1, pkt2)?;
                            settings_applied = true;
                        }
                        DeviceModel::Elgato4KX => {
                            eprintln!("Error: Audio input selection is only supported on 4K S");
                            return Err("Unsupported feature".into());
                        }
                    }
                } else {
                    eprintln!("Error: Invalid value '{}' for --audio-input", value);
                    eprintln!("Valid values: embedded, analog");
                    return Err("Invalid argument".into());
                }
            }
            "--video-scaler" => {
                if let Some(scaler) = VideoScaler::from_str(value) {
                    match device.model() {
                        DeviceModel::Elgato4KS => {
                            println!("Setting video scaler to {:?}", scaler);
                            let (pkt1, pkt2) = scaler.payload_4ks();
                            device.send_hid_two_packet(pkt1, pkt2)?;
                            settings_applied = true;
                        }
                        DeviceModel::Elgato4KX => {
                            eprintln!("Error: Video scaler is only supported on 4K S");
                            return Err("Unsupported feature".into());
                        }
                    }
                } else {
                    eprintln!("Error: Invalid value '{}' for --video-scaler", value);
                    eprintln!("Valid values: on, off");
                    return Err("Invalid argument".into());
                }
            }
            "--usb-speed" => {
                if let Some(speed) = UsbSpeed::from_str(value) {
                    match device.model() {
                        DeviceModel::Elgato4KX => {
                            println!("Setting USB speed to {:?}", speed);
                            println!("WARNING: Device will disconnect and re-enumerate with a different PID!");
                            // AT command 0x8e: Set USB speed
                            device.send_at_command(0x8e, &speed.at_input())?;
                            settings_applied = true;
                        }
                        DeviceModel::Elgato4KS => {
                            eprintln!("Error: USB speed switching is not supported on 4K S");
                            return Err("Unsupported feature".into());
                        }
                    }
                } else {
                    eprintln!("Error: Invalid value '{}' for --usb-speed", value);
                    eprintln!("Valid values: 5g, 10g");
                    return Err("Invalid argument".into());
                }
            }
            _ => {
                eprintln!("Error: Unknown option '{}'", arg);
                print_usage();
                return Err("Invalid argument".into());
            }
        }

        i += 2;
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    if settings_applied {
        println!("\n✓ All settings applied successfully!");
    } else {
        println!("No settings were changed.");
    }

    Ok(())
}
