//! UVC Extension Unit transport for the 4K X.
//!
//! The 4K X uses UVC XU #4 (GUID `961073c7-49f7-44f2-ab42-e940405940c2`) on
//! Interface 0. Normal settings use a two-step write: trigger (selector 0x02)
//! then payload (selector 0x01). Realtek's firmware updater instead sends its
//! command/data packets through the extra-I2C writer on selector 0x04.
//!
//! **Read protocol** (observed in Windows pcaps):
//!   1. SET_CUR sel 2 (trigger with payload length)
//!   2. SET_CUR sel 1 (probe/command payload)
//!   3. GET_LEN sel 1 (query response buffer size — changes dynamically)
//!   4. GET_CUR sel 1 (read response with exact length from GET_LEN)

use crate::device::ElgatoDevice;
use crate::error::ElgatoError;
use crate::protocol::*;
use crate::settings::DeviceModel;

// ---------------------------------------------------------------------------
// AT command framing (pure functions, testable without hardware)
// ---------------------------------------------------------------------------

/// Compute the LRC (Longitudinal Redundancy Check) for a byte slice.
///
/// LRC = two's complement of the sum of all bytes (mod 256).
fn lrc(data: &[u8]) -> u8 {
    let sum: u8 = data.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
    0u8.wrapping_sub(sum)
}

/// Build a framed AT command payload for the Realtek UVC protocol.
///
/// Returns `[0xa1, length_indicator, 0x00, 0x00, cmd_id(4B LE), input..., LRC]`.
pub(crate) fn frame_at_command(cmd_id: u32, input: &[u8]) -> Vec<u8> {
    // Combined data: [cmd_id as u32 LE] + [input_data]
    let mut data = cmd_id.to_le_bytes().to_vec();
    data.extend_from_slice(input);

    // Frame: [0xa1, length_indicator, 0x00, 0x00, data..., LRC]
    let length_indicator = ((data.len() + 2) & 0x7f) as u8;
    let mut payload = vec![0xa1, length_indicator, 0x00, 0x00];
    payload.extend_from_slice(&data);
    payload.push(lrc(&payload));
    payload
}

pub(crate) fn decode_at_response(response: &[u8]) -> Result<&[u8], ElgatoError> {
    if response.len() < 5 || response[0] != 0xa1 || response[3] != 0 {
        return Err(ElgatoError::UvcTransfer(
            "invalid AT response header".into(),
        ));
    }
    let extended = response[1] & 0x80 != 0;
    let encoded_length = if extended {
        (usize::from(response[1] & 0x7f) << 8) | usize::from(response[2])
    } else {
        usize::from(response[1])
    };
    // Short frames count two framing bytes; extended frames count only the
    // payload plus LRC. E.g. the four-byte HDMI state is a1 06 00 00 + data + LRC.
    let overhead = if extended { 4 } else { 3 };
    if encoded_length == 0 || response.len() != encoded_length + overhead {
        return Err(ElgatoError::UvcTransfer(format!(
            "AT response length mismatch: received {}, encoded {}, prefix {:02x?}",
            response.len(),
            encoded_length,
            &response[..response.len().min(16)]
        )));
    }
    if response
        .iter()
        .fold(0u8, |sum, byte| sum.wrapping_add(*byte))
        != 0
    {
        return Err(ElgatoError::UvcTransfer(
            "AT response LRC is invalid".into(),
        ));
    }
    Ok(&response[4..response.len() - 1])
}

/// Build a family 0x06 AT read probe: `[a1, 06, 00, 00, sub_cmd, 00, 00, 00, LRC]`.
pub(crate) fn frame_at_read_probe(sub_cmd: u8) -> Vec<u8> {
    let mut payload = vec![0xa1, 0x06, 0x00, 0x00, sub_cmd, 0x00, 0x00, 0x00];
    payload.push(lrc(&payload));
    payload
}

/// Build a family 0x07 AT read probe: `[a1, 07, 00, 00, sub_cmd, 00, 00, 00, param, LRC]`.
pub(crate) fn frame_at_read_probe_family07(sub_cmd: u8, param: u8) -> Vec<u8> {
    let mut payload = vec![0xa1, 0x07, 0x00, 0x00, sub_cmd, 0x00, 0x00, 0x00, param];
    payload.push(lrc(&payload));
    payload
}

/// UVC Extension Unit protocol methods for the 4K X.
///
/// Uses XU #4 with GUID `961073c7-49f7-44f2-ab42-e940405940c2`.
/// Every setting change uses a two-step write:
///   1. SET_CUR → selector 0x02 (trigger)
///   2. SET_CUR → selector 0x01 (payload)
impl ElgatoDevice {
    // --- Low-level UVC transport ---

    /// Send a trigger with arbitrary data to selector 0x02.
    ///
    /// The trigger announces the byte count of the payload that follows on
    /// selector 0x01.  Both `a1 XX` setting writes and AT commands use this
    /// same length-announcement mechanism.
    pub(crate) fn send_uvc_trigger_data(&self, data: &[u8]) -> Result<(), ElgatoError> {
        let w_value = UVC_SELECTOR_TRIGGER << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE;

        self.handle.write_control(
            UVC_REQUEST_TYPE_OUT,
            UVC_SET_CUR,
            w_value,
            w_index,
            data,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::UvcTransfer(format!("trigger SET_CUR failed: {}", e)))?;

        Ok(())
    }

    /// Send a payload to selector 0x01.
    pub(crate) fn send_uvc_payload(&self, payload: &[u8]) -> Result<(), ElgatoError> {
        let w_value = UVC_SELECTOR_VALUE << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE;

        self.handle.write_control(
            UVC_REQUEST_TYPE_OUT,
            UVC_SET_CUR,
            w_value,
            w_index,
            payload,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::UvcTransfer(format!("payload SET_CUR failed: {}", e)))?;

        Ok(())
    }

    /// Write a setting and complete the device's write/status/read transaction.
    /// RTICE's `rtk_sendATCommand` always performs this response read; without
    /// it EDID mode changes can be left unapplied by the firmware.
    ///
    /// The trigger announces the payload length as a u16 LE value, matching
    /// the Windows driver behavior observed in USB captures.
    pub(crate) fn set_uvc_setting(&self, payload: &[u8]) -> Result<(), ElgatoError> {
        self.probe_uvc_setting(payload).map(|_| ())
    }

    fn write_uvc_setting(&self, payload: &[u8]) -> Result<(), ElgatoError> {
        let trigger = (payload.len() as u16).to_le_bytes();
        self.send_uvc_trigger_data(&trigger)?;
        self.send_uvc_payload(payload)?;
        Ok(())
    }

    /// GET_LEN on a selector — returns the current descriptor length.
    ///
    /// The device dynamically changes this value after a SET_CUR to reflect
    /// the size of the response buffer. Windows always queries this before
    /// GET_CUR and uses the returned value as wLength.
    pub(crate) fn get_uvc_len(&self, selector: u16) -> Result<u16, ElgatoError> {
        let w_value = selector << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE;
        let mut buf = [0u8; 2];

        let len = self.handle.read_control(
            UVC_REQUEST_TYPE_IN,
            UVC_GET_LEN,
            w_value,
            w_index,
            &mut buf,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::UvcTransfer(format!("GET_LEN failed: {}", e)))?;

        if len < 2 {
            return Err(ElgatoError::UvcTransfer(format!("GET_LEN returned {} bytes", len)));
        }

        Ok(u16::from_le_bytes(buf))
    }

    /// GET_CUR on selector 0x01 with a specific buffer size.
    pub(crate) fn read_uvc_raw(&self, length: usize) -> Result<Vec<u8>, ElgatoError> {
        let w_value = UVC_SELECTOR_VALUE << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE;
        let mut buf = vec![0u8; length];

        let len = self.handle.read_control(
            UVC_REQUEST_TYPE_IN,
            UVC_GET_CUR,
            w_value,
            w_index,
            &mut buf,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::UvcTransfer(format!("GET_CUR failed: {}", e)))?;

        buf.truncate(len);
        Ok(buf)
    }

    /// GET_CUR on selector 0x01 using GET_LEN to determine the buffer size.
    ///
    /// Queries GET_LEN first to get the current descriptor length, then reads
    /// with exactly that size. This matches the Windows driver behavior.
    pub(crate) fn read_uvc_setting(&self) -> Result<Vec<u8>, ElgatoError> {
        let response_len = self.get_uvc_len(UVC_SELECTOR_VALUE)? as usize;
        self.read_uvc_raw(response_len)
    }

    /// Read GET_CUR on selector 0x02 (trigger/status register).
    ///
    /// Windows polls this after every SET_CUR on sel 1 before reading the
    /// response. This gives the device time to process the command and
    /// update the response buffer + GET_LEN descriptor.
    pub(crate) fn poll_uvc_status(&self) -> Result<Vec<u8>, ElgatoError> {
        let response_len = self.get_uvc_len(UVC_SELECTOR_TRIGGER)? as usize;
        let w_value = UVC_SELECTOR_TRIGGER << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE;
        let mut buf = vec![0u8; response_len];

        let len = self.handle.read_control(
            UVC_REQUEST_TYPE_IN,
            UVC_GET_CUR,
            w_value,
            w_index,
            &mut buf,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::UvcTransfer(format!("status GET_CUR failed: {}", e)))?;

        buf.truncate(len);
        Ok(buf)
    }

    /// Write a probe payload, then read back the response using GET_LEN.
    ///
    /// Full sequence matching Windows pcaps:
    ///   1. SET_CUR sel 2 (trigger with payload length)
    ///   2. SET_CUR sel 1 (probe payload)
    ///   3. GET_LEN sel 2 + GET_CUR sel 2 (status poll — gives device processing time)
    ///   4. GET_LEN sel 1 (query dynamic response size)
    ///   5. GET_CUR sel 1 (read response)
    pub(crate) fn probe_uvc_setting(&self, probe: &[u8]) -> Result<Vec<u8>, ElgatoError> {
        self.write_uvc_setting(probe)?;
        // Poll sel 2 status — matches Windows behavior and gives the device
        // time to process the command before we query GET_LEN on sel 1
        self.poll_uvc_status()?;
        self.read_uvc_setting()
    }

    // --- AT Command framing ---
    //
    // The ITE UB700E chip uses "AT commands" internally. In the UVC protocol,
    // all AT commands (both reads and writes) use the same framing:
    //   [family_byte, length_indicator, 0x00, 0x00, cmd_id(4B LE), input..., LRC]
    //
    // From Ghidra RE of libRTK_IO_x64.dylib (CRosRTD2801Protocol::_sendATCommand):
    //   family_byte = cmd_type + 0xa0 (cmd_type=1 → 0xa1)
    //   length_indicator = (combined_data_len + 2) & 0x7f
    //   combined_data = [cmd_id as u32 LE] + [input_data]
    //   LRC = two's complement of sum of all preceding bytes

    /// Send a framed AT command and read the ACK response (4K X only).
    ///
    /// Builds the `a1 XX 00 00 cmd_id ... LRC` framed payload matching the
    /// Realtek protocol used by the official software, then reads back the
    /// device response. The Mac library always performs a write+read cycle
    /// for AT commands — the device may not commit changes until the
    /// response is read.
    pub(crate) fn send_at_command(&self, cmd_id: u32, input: &[u8]) -> Result<Vec<u8>, ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "AT commands",
                model: "4K S",
            });
        }

        let payload = frame_at_command(cmd_id, input);
        self.probe_uvc_setting(&payload)
    }

    /// Read the complete seven-slot custom EDID flash table.
    pub fn read_edid_table(&self) -> Result<Vec<u8>, ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "Custom EDID table reading",
                model: "4K S",
            });
        }
        const CHUNK_SIZE: usize = 0x80;
        const TABLE_SIZE: usize = 0xe00;
        let mut table = Vec::with_capacity(TABLE_SIZE);
        for offset in (0..TABLE_SIZE).step_by(CHUNK_SIZE) {
            let mut input = Vec::with_capacity(4);
            input.extend_from_slice(&(offset as u16).to_le_bytes());
            input.extend_from_slice(&(CHUNK_SIZE as u16).to_le_bytes());
            let response = self.send_at_command(0x54, &input)?;
            let chunk = decode_at_response(&response)?;
            if chunk.len() != CHUNK_SIZE {
                return Err(ElgatoError::UvcTransfer(
                    "EDID table chunk size mismatch".into(),
                ));
            }
            table.extend_from_slice(chunk);
        }
        Ok(table)
    }

    pub(crate) fn enter_update_mode(&self, packet: &[u8]) -> Result<(), ElgatoError> {
        if let Err(error) = self.write_update_payload(packet) {
            if error.to_string().contains("No such device") {
                return Ok(());
            }
            return Err(error);
        }

        let mut last_response = Vec::new();
        for _ in 0..40 {
            match self.read_update_response() {
                Ok(response) if response.first().copied() == Some(0x06) => return Ok(()),
                Ok(response) => last_response = response,
                Err(error) if error.to_string().contains("No such device") => return Ok(()),
                Err(error) => return Err(error),
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        Err(ElgatoError::UvcTransfer(format!(
            "enter_rescue neither acknowledged nor disconnected: {:02x?}",
            last_response
        )))
    }

    /// RTK_IO routes updater writes through CProtocol::extraI2CSendBytes and
    /// CUVCExtensionPort's selector-4 write method.
    fn write_update_payload(&self, payload: &[u8]) -> Result<(), ElgatoError> {
        let written = self
            .handle
            .write_control(
                UVC_REQUEST_TYPE_OUT,
                UVC_SET_CUR,
                UVC_SELECTOR_EXTRA_I2C_WRITE << 8,
                (UVC_ENTITY_ID << 8) | UVC_INTERFACE,
                payload,
                USB_TIMEOUT,
            )
            .map_err(|e| {
                ElgatoError::UvcTransfer(format!("updater selector-4 SET_CUR failed: {}", e))
            })?;
        if written != payload.len() {
            return Err(ElgatoError::UvcTransfer(format!(
                "updater selector-4 SET_CUR wrote {} of {} bytes",
                written,
                payload.len()
            )));
        }
        Ok(())
    }

    /// Read the updater ACK through CUVCExtensionPort's extra-I2C read method.
    /// The native implementation pairs extraI2CSendBytes (selector 4 SET)
    /// with extraI2CReadBytes (selector 4 GET), using a 0x400-byte buffer.
    fn read_update_response(&self) -> Result<Vec<u8>, ElgatoError> {
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE;
        let mut response = vec![0u8; 0x400];
        let response_len = self
            .handle
            .read_control(
                UVC_REQUEST_TYPE_IN,
                UVC_GET_CUR,
                UVC_SELECTOR_EXTRA_I2C_WRITE << 8,
                w_index,
                &mut response,
                USB_TIMEOUT,
            )
            .map_err(|e| {
                ElgatoError::UvcTransfer(format!("updater response GET_CUR failed: {}", e))
            })?;
        response.truncate(response_len);
        Ok(response)
    }

    /// Read an AT command response via `a1 06` family probe (4K X only).
    ///
    /// Sends a family 0x06 probe with the sub-command ID at byte[4], then
    /// reads back the response using GET_LEN + GET_CUR. Response is typically
    /// 133 bytes with a `a1 80 XX 00` header followed by data.
    pub(crate) fn read_at_command(&self, sub_cmd: u8) -> Result<Vec<u8>, ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "AT commands",
                model: "4K S",
            });
        }

        self.probe_uvc_setting(&frame_at_read_probe(sub_cmd))
    }

    /// Read an AT command response via `a1 07` family probe (4K X only).
    ///
    /// Family 0x07 probes are 10 bytes with an extra parameter byte at [8].
    /// Used for EDID Range Policy reads (sub-cmd 0x91, param 0x01).
    pub(crate) fn read_at_command_family07(&self, sub_cmd: u8, param: u8) -> Result<Vec<u8>, ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "AT commands",
                model: "4K S",
            });
        }

        self.probe_uvc_setting(&frame_at_read_probe_family07(sub_cmd, param))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lrc_checksum() {
        // LRC = two's complement of byte sum (mod 256)
        assert_eq!(lrc(&[0xa1, 0x06, 0x00, 0x00, 0x77, 0x00, 0x00, 0x00]), 0xe2);
    }

    #[test]
    fn frame_at_read_probe_firmware() {
        // Firmware version read: AT cmd 0x77 via a1 06 family
        let payload = frame_at_read_probe(0x77);
        assert_eq!(payload, vec![0xa1, 0x06, 0x00, 0x00, 0x77, 0x00, 0x00, 0x00, 0xe2]);
        // Verify LRC: sum of all bytes should be 0 (mod 256)
        let total: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(total, 0);
    }

    #[test]
    fn frame_at_read_probe_hdr() {
        // HDR read: AT cmd 0x90
        let payload = frame_at_read_probe(0x90);
        assert_eq!(payload.len(), 9);
        assert_eq!(payload[0], 0xa1);
        assert_eq!(payload[1], 0x06);
        assert_eq!(payload[4], 0x90);
        let total: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(total, 0);
    }

    #[test]
    fn frame_at_read_probe_family07_color_range() {
        // EDID range policy read: AT cmd 0x91, family 0x07, param 0x01
        let payload = frame_at_read_probe_family07(0x91, 0x01);
        assert_eq!(payload.len(), 10);
        assert_eq!(payload[0], 0xa1);
        assert_eq!(payload[1], 0x07);
        assert_eq!(payload[4], 0x91);
        assert_eq!(payload[8], 0x01);
        let total: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(total, 0);
    }

    #[test]
    fn frame_at_command_usb_speed_10g() {
        // USB speed 10Gbps: AT cmd 0x8e, input [01 00 00 00 03 00 00 00]
        let input = [0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00];
        let payload = frame_at_command(0x8e, &input);
        // Expected: a1 0e 00 00 | 8e 00 00 00 | 01 00 00 00 03 00 00 00 | LRC
        assert_eq!(payload.len(), 17);
        assert_eq!(payload[0], 0xa1); // family byte
        assert_eq!(payload[1], 0x0e); // length_indicator: (12+2) & 0x7f = 14 = 0x0e
        assert_eq!(payload[4], 0x8e); // cmd_id byte 0
        assert_eq!(payload[8], 0x01); // constant from input
        assert_eq!(payload[12], 0x03); // speed value
        let total: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(total, 0);
    }

    #[test]
    fn frame_at_command_usb_speed_5g() {
        // USB speed 5Gbps: AT cmd 0x8e, input [01 00 00 00 00 00 00 00]
        let input = [0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let payload = frame_at_command(0x8e, &input);
        assert_eq!(payload.len(), 17);
        assert_eq!(payload[1], 0x0e);
        assert_eq!(payload[12], 0x00); // speed value = 5Gbps
        let total: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(total, 0);
    }

    #[test]
    fn frame_at_command_empty_input() {
        // AT command with no extra input (just cmd_id)
        let payload = frame_at_command(0x67, &[]);
        // Expected: a1 06 00 00 | 67 00 00 00 | LRC
        assert_eq!(payload.len(), 9);
        assert_eq!(payload[1], 0x06); // length_indicator: (4+2) & 0x7f = 6
        assert_eq!(payload[4], 0x67);
        let total: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        assert_eq!(total, 0);
    }

    #[test]
    fn decode_extended_at_response() {
        let data = vec![0x5a; 0x80];
        let mut response = vec![0xa1, 0x80, 0x81, 0x00];
        response.extend_from_slice(&data);
        response.push(lrc(&response));
        assert_eq!(decode_at_response(&response).unwrap(), data);
    }

    #[test]
    fn decode_at_response_rejects_bad_length_and_lrc() {
        let mut response = vec![0xa1, 0x06, 0x00, 0x00, 1, 2, 3, 4];
        response.push(lrc(&response));
        assert!(decode_at_response(&response).is_ok());
        response[1] = 7;
        assert!(decode_at_response(&response).is_err());
        response[1] = 6;
        response[4] ^= 1;
        assert!(decode_at_response(&response).is_err());
    }

    #[test]
    fn decode_captured_short_hdmi_state_response() {
        let response = [0xa1, 0x06, 0x00, 0x00, 1, 0, 0, 0, 0x58];
        assert_eq!(decode_at_response(&response).unwrap(), [1, 0, 0, 0]);
        assert!(decode_at_response(&response[..8]).is_err());
    }
}
