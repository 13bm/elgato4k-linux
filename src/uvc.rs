//! UVC Extension Unit transport for the 4K X.
//!
//! The 4K X uses UVC XU #4 (GUID `961073c7-49f7-44f2-ab42-e940405940c2`) on
//! Interface 0.  Every setting change is a two-step write: trigger (selector
//! 0x02) then payload (selector 0x01).
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

    /// Two-step write: length trigger + payload (for `a1 XX` style commands).
    ///
    /// The trigger announces the payload length as a u16 LE value, matching
    /// the Windows driver behavior observed in USB captures.
    pub(crate) fn set_uvc_setting(&self, payload: &[u8]) -> Result<(), ElgatoError> {
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
        self.set_uvc_setting(probe)?;
        // Poll sel 2 status — matches Windows behavior and gives the device
        // time to process the command before we query GET_LEN on sel 1
        let _ = self.poll_uvc_status();
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

        // Combined data: [cmd_id as u32 LE] + [input_data]
        let mut data = cmd_id.to_le_bytes().to_vec();
        data.extend_from_slice(input);

        // Frame: [0xa1, length_indicator, 0x00, 0x00, data..., LRC]
        let length_indicator = ((data.len() + 2) & 0x7f) as u8;
        let mut payload = vec![0xa1, length_indicator, 0x00, 0x00];
        payload.extend_from_slice(&data);
        let sum: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        payload.push(0u8.wrapping_sub(sum));

        // Write + poll + read (same as probe_uvc_setting)
        self.probe_uvc_setting(&payload)
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

        // Build a1 06 family probe: [a1, 06, 00, 00, sub_cmd, 00, 00, 00, checksum]
        let mut payload = vec![0xa1, 0x06, 0x00, 0x00, sub_cmd, 0x00, 0x00, 0x00];
        let sum: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        payload.push(0u8.wrapping_sub(sum));

        self.probe_uvc_setting(&payload)
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

        // Build a1 07 family probe: [a1, 07, 00, 00, sub_cmd, 00, 00, 00, param, checksum]
        let mut payload = vec![0xa1, 0x07, 0x00, 0x00, sub_cmd, 0x00, 0x00, 0x00, param];
        let sum: u8 = payload.iter().fold(0u8, |acc, &b| acc.wrapping_add(b));
        payload.push(0u8.wrapping_sub(sum));

        self.probe_uvc_setting(&payload)
    }
}
