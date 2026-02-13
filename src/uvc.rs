//! UVC Extension Unit transport for the 4K X.
//!
//! The 4K X uses UVC XU #4 (GUID `961073c7-49f7-44f2-ab42-e940405940c2`) on
//! Interface 0.  Every setting change is a two-step write: trigger (selector
//! 0x02) then payload (selector 0x01).  AT commands use the same selectors
//! with length-prefixed framing.

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

    /// Send the standard trigger (selector 0x02) with fixed data `[0x09, 0x00]`.
    pub(crate) fn send_uvc_trigger(&self) -> Result<(), ElgatoError> {
        self.send_uvc_trigger_data(&[0x09, 0x00])
    }

    /// Send a trigger with arbitrary data (used by AT commands for length encoding).
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

    /// Standard two-step: fixed trigger + payload (for `a1 XX` style commands).
    pub(crate) fn set_uvc_setting(&self, payload: &[u8]) -> Result<(), ElgatoError> {
        self.send_uvc_trigger()?;
        self.send_uvc_payload(payload)?;
        Ok(())
    }

    /// Read current value from selector 0x01 via GET_CUR.
    pub(crate) fn read_uvc_setting(&self, length: usize) -> Result<Vec<u8>, ElgatoError> {
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

    // --- AT Command framing ---
    //
    // The ITE UB700E chip uses "AT commands" internally. The framing is:
    //   Trigger data = [payload_length as u16 LE]
    //   Payload      = [cmd_id as u32 LE] + [input_data]
    //
    // This goes through the same UVC XU #4, selectors 0x02 and 0x01.

    /// Send an AT command (4K X only).
    pub(crate) fn send_at_command(&self, cmd_id: u32, input: &[u8]) -> Result<(), ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "AT commands",
                model: "4K S",
            });
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
    pub(crate) fn read_at_command(&self, cmd_id: u32, response_len: usize) -> Result<Vec<u8>, ElgatoError> {
        if self.model != DeviceModel::Elgato4KX {
            return Err(ElgatoError::UnsupportedFeature {
                feature: "AT commands",
                model: "4K S",
            });
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
