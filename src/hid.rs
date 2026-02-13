//! HID SET_REPORT / GET_REPORT transport for the 4K S.
//!
//! All communication with the 4K S uses 255-byte zero-padded HID reports on
//! Interface 7.  Write operations use SET_REPORT (Output), read operations
//! send a SET_REPORT request followed by GET_REPORT (Input).

use crate::device::ElgatoDevice;
use crate::error::ElgatoError;
use crate::protocol::*;

/// HID Output/Input Report protocol methods for the 4K S.
///
/// Uses SET_REPORT/GET_REPORT requests on Interface 7 with 255-byte zero-padded packets.
/// Write header format: `06 06 06 55 [cmd bytes...]`
/// Read request format: `06 55 [sub_cmd] [data_len]` (then GET_REPORT to receive response)
impl ElgatoDevice {
    /// Send a single HID output report (must be exactly [`HID_PACKET_SIZE`] bytes).
    pub(crate) fn send_hid_packet(&self, packet: &[u8]) -> Result<(), ElgatoError> {
        if packet.len() != HID_PACKET_SIZE {
            return Err(ElgatoError::HidPacketSize {
                expected: HID_PACKET_SIZE,
                got: packet.len(),
            });
        }

        self.handle.write_control(
            HID_REQUEST_TYPE_OUT,
            HID_SET_REPORT,
            HID_REPORT_VALUE_OUTPUT,
            HID_INTERFACE,
            packet,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::HidTransfer(format!("SET_REPORT failed: {}", e)))?;

        Ok(())
    }

    /// Send a two-packet HID sequence with a short inter-packet delay.
    pub(crate) fn send_hid_two_packet(
        &self,
        pkt1: &[u8; HID_PACKET_SIZE],
        pkt2: &[u8; HID_PACKET_SIZE],
    ) -> Result<(), ElgatoError> {
        self.send_hid_packet(pkt1)?;
        std::thread::sleep(HID_INTER_PACKET_DELAY);
        self.send_hid_packet(pkt2)?;
        Ok(())
    }

    /// Read data from the 4K S by sending a HID read request then GET_REPORT.
    ///
    /// This implements the ReadI2cData protocol from EGAVDeviceSupport:
    ///   1. SET_REPORT with `[report_id, cmd, sub_cmd, data_len]` to tell device what to send
    ///   2. GET_REPORT (Input) to read back the response
    ///
    /// Returns the raw response bytes (after the report ID byte).
    pub(crate) fn read_hid_data(&self, cmd: u8, sub_cmd: u8, data_len: u8) -> Result<Vec<u8>, ElgatoError> {
        // Build the read request packet on the stack
        let mut request = [0u8; HID_PACKET_SIZE];
        request[0] = HID_REPORT_ID;
        request[1] = cmd;
        request[2] = sub_cmd;
        request[3] = data_len;

        // Send the request via SET_REPORT (Output)
        self.handle.write_control(
            HID_REQUEST_TYPE_OUT,
            HID_SET_REPORT,
            HID_REPORT_VALUE_OUTPUT,
            HID_INTERFACE,
            &request,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::HidTransfer(format!("read request SET_REPORT failed: {}", e)))?;

        // Small delay for device to prepare response
        std::thread::sleep(HID_READ_DELAY);

        // Read back via GET_REPORT (Input)
        let mut buf = [0u8; HID_PACKET_SIZE];
        buf[0] = HID_REPORT_ID; // Report ID must be set in buffer for GET_REPORT

        let len = self.handle.read_control(
            HID_REQUEST_TYPE_IN,
            HID_GET_REPORT,
            HID_REPORT_VALUE_INPUT,
            HID_INTERFACE,
            &mut buf,
            USB_TIMEOUT,
        ).map_err(|e| ElgatoError::HidTransfer(format!("GET_REPORT failed: {}", e)))?;

        // Return data after report ID byte
        if len > 1 {
            Ok(buf[1..len].to_vec())
        } else {
            Ok(vec![])
        }
    }
}
