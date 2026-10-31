//! HDMI receiver timing, independent of the negotiated USB capture format.

use std::fmt;

use crate::device::ElgatoDevice;
use crate::error::ElgatoError;
use crate::protocol::*;
use crate::settings::DeviceModel;
use crate::uvc::decode_at_response;

/// HDMI input timing from RTICE's 40-byte raw receiver structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HdmiTiming {
    pub width: u16,
    pub height: u16,
    /// Receiver refresh rate in hundredths of a hertz, not USB delivery FPS.
    pub refresh_rate_centi_hz: u16,
    pub interlaced: bool,
}

impl HdmiTiming {
    /// Decode the same fields consumed by CCUVC's 4K X format-polling path.
    pub fn from_raw(data: &[u8]) -> Result<Self, ElgatoError> {
        if data.len() < 40 {
            return Err(ElgatoError::UvcTransfer(format!(
                "HDMI raw timing needs 40 bytes, received {}",
                data.len()
            )));
        }
        let word = |offset| u16::from_le_bytes([data[offset], data[offset + 1]]);
        let timing = Self {
            width: word(8),
            height: word(6),
            refresh_rate_centi_hz: word(14),
            interlaced: data[0] & 1 != 0,
        };
        if timing.width == 0 || timing.height == 0 || timing.refresh_rate_centi_hz == 0 {
            return Err(ElgatoError::UvcTransfer(
                "HDMI timing contains zero dimensions or refresh rate".into(),
            ));
        }
        Ok(timing)
    }

    pub fn refresh_rate_hz(&self) -> f64 {
        f64::from(self.refresh_rate_centi_hz) / 100.0
    }
}

impl fmt::Display for HdmiTiming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}x{}{} @ {:.2} Hz",
            self.width,
            self.height,
            if self.interlaced { "i" } else { "p" },
            self.refresh_rate_hz()
        )
    }
}

/// A receiver without lock must not report a stale resolution as current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HdmiSignal {
    Disconnected,
    NoSignal,
    /// Lock changed while reading; retry once the source has settled.
    Changing,
    Locked(HdmiTiming),
}

impl fmt::Display for HdmiSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::NoSignal => write!(f, "no signal (receiver not locked)"),
            Self::Changing => write!(f, "signal changing (retry)"),
            Self::Locked(timing) => timing.fmt(f),
        }
    }
}

fn read_flag(response: &[u8]) -> Result<bool, ElgatoError> {
    let data = decode_at_response(response)?;
    let bytes = data
        .get(..4)
        .ok_or_else(|| ElgatoError::UvcTransfer("HDMI state response needs four bytes".into()))?;
    match u32::from_le_bytes(bytes.try_into().unwrap()) {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(ElgatoError::UvcTransfer(format!(
            "unknown HDMI state {value}"
        ))),
    }
}

pub(crate) fn read_signal(
    mut query: impl FnMut(u32) -> Result<Vec<u8>, ElgatoError>,
) -> Result<HdmiSignal, ElgatoError> {
    if !read_flag(&query(AT_CMD_HDMI_CABLE_CONNECTED)?)? {
        return Ok(HdmiSignal::Disconnected);
    }
    // CCUVC polls raw timing before video stability on the 4K X.
    let response = query(AT_CMD_HDMI_RAW_TIMING)?;
    let raw = decode_at_response(&response)?;
    if !read_flag(&query(AT_CMD_HDMI_VIDEO_STABLE)?)? {
        return Ok(HdmiSignal::NoSignal);
    }
    // A cable disconnect may invalidate the timing while the query runs.
    if !read_flag(&query(AT_CMD_HDMI_CABLE_CONNECTED)?)? {
        return Ok(HdmiSignal::Changing);
    }
    Ok(HdmiSignal::Locked(HdmiTiming::from_raw(raw)?))
}

impl ElgatoDevice {
    /// Read the 4K X's HDMI input resolution and rate, not its UVC format.
    ///
    /// The queries do not change settings. Opening this device via libusb
    /// nevertheless detaches uvcvideo and interrupts active video capture.
    /// Use [`crate::read_hdmi_signal_v4l2`] instead for live capture queries.
    pub fn read_hdmi_signal(&self) -> Result<HdmiSignal, ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "HDMI input timing",
                model: "4K S",
            });
        }
        read_signal(|command| self.send_at_command(command, &[]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(width: u16, height: u16, rate: u16, interlaced: bool) -> Vec<u8> {
        let mut data = vec![0; 40];
        data[0] = u8::from(interlaced);
        data[6..8].copy_from_slice(&height.to_le_bytes());
        data[8..10].copy_from_slice(&width.to_le_bytes());
        data[14..16].copy_from_slice(&rate.to_le_bytes());
        data
    }

    fn response(data: &[u8]) -> Vec<u8> {
        let mut bytes = vec![0xa1, (data.len() + 2) as u8, 0, 0];
        bytes.extend_from_slice(data);
        bytes.push(0u8.wrapping_sub(bytes.iter().fold(0u8, |sum, b| sum.wrapping_add(*b))));
        bytes
    }

    #[test]
    fn windows_timing_offsets_and_rates() {
        for (width, height, rate, interlaced, text) in [
            (1920, 1080, 12000, false, "1920x1080p @ 120.00 Hz"),
            (3840, 2160, 5994, false, "3840x2160p @ 59.94 Hz"),
            (1920, 1080, 14450, false, "1920x1080p @ 144.50 Hz"),
            (1920, 1080, 6000, true, "1920x1080i @ 60.00 Hz"),
            (1920, 1080, 24000, false, "1920x1080p @ 240.00 Hz"),
            (2560, 1440, 14400, false, "2560x1440p @ 144.00 Hz"),
        ] {
            let timing = HdmiTiming::from_raw(&raw(width, height, rate, interlaced)).unwrap();
            assert_eq!(
                (
                    timing.width,
                    timing.height,
                    timing.refresh_rate_centi_hz,
                    timing.interlaced
                ),
                (width, height, rate, interlaced)
            );
            assert_eq!(timing.to_string(), text);
        }
    }

    #[test]
    fn rejects_short_and_empty_timings_without_panicking() {
        for length in 0..40 {
            assert!(HdmiTiming::from_raw(&vec![0; length]).is_err());
        }
        for data in [
            raw(0, 1080, 6000, false),
            raw(1920, 0, 6000, false),
            raw(1920, 1080, 0, false),
        ] {
            assert!(HdmiTiming::from_raw(&data).is_err());
        }
    }

    #[test]
    fn state_does_not_accept_short_unknown_or_corrupt_responses() {
        assert!(read_flag(&response(&[1])).is_err());
        assert!(read_flag(&response(&2u32.to_le_bytes())).is_err());
        let mut data = response(&1u32.to_le_bytes());
        data[4] ^= 1;
        assert!(read_flag(&data).is_err());
    }

    #[test]
    fn locked_reads_only_verified_query_commands() {
        let mut commands = Vec::new();
        let signal = read_signal(|command| {
            commands.push(command);
            Ok(response(&if command == AT_CMD_HDMI_RAW_TIMING {
                raw(1920, 1080, 12000, false)
            } else {
                1u32.to_le_bytes().to_vec()
            }))
        })
        .unwrap();
        assert_eq!(signal.to_string(), "1920x1080p @ 120.00 Hz");
        assert_eq!(commands, [0x36, 0x37, 0x38, 0x36]);
    }

    #[test]
    fn no_lock_never_uses_stale_timing() {
        for connected in [false, true] {
            let mut commands = Vec::new();
            let signal = read_signal(|command| {
                commands.push(command);
                Ok(response(&if command == 0x37 {
                    raw(1920, 1080, 12000, false)
                } else {
                    u32::from(command == 0x36 && connected)
                        .to_le_bytes()
                        .to_vec()
                }))
            })
            .unwrap();
            assert_eq!(
                signal,
                if connected {
                    HdmiSignal::NoSignal
                } else {
                    HdmiSignal::Disconnected
                }
            );
            assert_eq!(commands.contains(&0x37), connected);
        }
    }

    #[test]
    fn unplug_during_read_is_not_reported_as_current_timing() {
        let mut cable_reads = 0;
        let signal = read_signal(|command| {
            let data = match command {
                0x37 => raw(1920, 1080, 12000, false),
                0x36 => {
                    cable_reads += 1;
                    u32::from(cable_reads == 1).to_le_bytes().to_vec()
                }
                _ => 1u32.to_le_bytes().to_vec(),
            };
            Ok(response(&data))
        })
        .unwrap();
        assert_eq!(signal, HdmiSignal::Changing);
    }

    #[test]
    fn transport_errors_are_not_disguised_as_no_signal() {
        assert!(read_signal(|_| Err(ElgatoError::Usb(rusb::Error::Timeout))).is_err());
    }
}
