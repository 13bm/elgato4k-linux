use crate::device::ElgatoDevice;
use crate::settings::DeviceModel;
use std::time::Duration;

// 4K X (UVC) Control Request parameters
const UVC_REQUEST_TYPE: u8 = 0x21;
const UVC_REQUEST_SET_CUR: u8 = 0x01;
pub const UVC_INTERFACE_NUM: u16 = 0;
const UVC_ENTITY_ID: u16 = 4;
const UVC_SELECTOR_TRIGGER: u16 = 0x02;
const UVC_SELECTOR_VALUE: u16 = 0x01;

const TIMEOUT: Duration = Duration::from_secs(1);

/// UVC Extension Unit protocol methods for the 4K X.
///
/// Uses XU #4 with GUID 961073c7-49f7-44f2-ab42-e940405940c2.
/// Every setting change uses a two-step write:
///   1. SET_CUR -> selector 0x02 (trigger)
///   2. SET_CUR -> selector 0x01 (payload)
impl ElgatoDevice {
    // --- Low-level UVC transport ---

    /// Send the standard trigger (selector 0x02) with fixed data [0x09, 0x00].
    pub(crate) fn send_uvc_trigger(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_uvc_trigger_data(&[0x09, 0x00])
    }

    /// Send a trigger with arbitrary data (used by AT commands for length encoding).
    pub(crate) fn send_uvc_trigger_data(&self, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let w_value = UVC_SELECTOR_TRIGGER << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE_NUM;

        self.handle.write_control(
            UVC_REQUEST_TYPE,
            UVC_REQUEST_SET_CUR,
            w_value,
            w_index,
            data,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send UVC trigger: {}", e))?;

        Ok(())
    }

    /// Send a payload to selector 0x01.
    pub(crate) fn send_uvc_payload(&self, payload: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let w_value = UVC_SELECTOR_VALUE << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE_NUM;

        self.handle.write_control(
            UVC_REQUEST_TYPE,
            UVC_REQUEST_SET_CUR,
            w_value,
            w_index,
            payload,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send UVC payload: {}", e))?;

        Ok(())
    }

    /// Standard two-step: fixed trigger + payload (for a1 XX style commands).
    pub(crate) fn set_uvc_setting(&self, payload: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        self.send_uvc_trigger()?;
        self.send_uvc_payload(&payload)?;
        Ok(())
    }

    /// Read current value from selector 0x01 via GET_CUR.
    pub(crate) fn read_uvc_setting(&self, length: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let w_value = UVC_SELECTOR_VALUE << 8;
        let w_index = (UVC_ENTITY_ID << 8) | UVC_INTERFACE_NUM;
        let mut buf = vec![0u8; length];

        match self.handle.read_control(
            0xa1, // Device-to-host, Class, Interface
            0x81, // GET_CUR
            w_value,
            w_index,
            &mut buf,
            TIMEOUT,
        ) {
            Ok(len) => Ok(buf[..len].to_vec()),
            Err(e) => Err(format!("Failed to read setting: {}", e).into()),
        }
    }

    // --- AT Command framing ---
    //
    // The ITE UB700E chip uses "AT commands" internally. The framing is:
    //   Trigger data = [payload_length as u16 LE]
    //   Payload      = [cmd_id as u32 LE] + [input_data]
    //
    // This goes through the same UVC XU #4, selectors 0x02 and 0x01.

    /// Send an AT command (4K X only).
    pub fn send_at_command(&self, cmd_id: u32, input: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        match self.model {
            DeviceModel::Elgato4KX => {}
            DeviceModel::Elgato4KS => {
                return Err("AT commands are only supported on 4K X (UVC)".into());
            }
        }

        // Build payload: [cmd_id 4B LE] + [input data]
        let mut payload = cmd_id.to_le_bytes().to_vec();
        payload.extend_from_slice(input);

        // Trigger with total payload length as u16 LE
        let trigger = (payload.len() as u16).to_le_bytes();
        self.send_uvc_trigger_data(&trigger)?;
        self.send_uvc_payload(&payload)?;

        Ok(())
    }

    /// Read an AT command response (4K X only).
    /// Sends the AT command with empty input, then reads back the response.
    pub fn read_at_command(&self, cmd_id: u32, response_len: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        match self.model {
            DeviceModel::Elgato4KX => {}
            DeviceModel::Elgato4KS => {
                return Err("AT commands are only supported on 4K X (UVC)".into());
            }
        }

        // Send the command with no input data
        let payload = cmd_id.to_le_bytes().to_vec();
        let trigger = (payload.len() as u16).to_le_bytes();
        self.send_uvc_trigger_data(&trigger)?;
        self.send_uvc_payload(&payload)?;

        // Read back the response
        self.read_uvc_setting(response_len)
    }
}
