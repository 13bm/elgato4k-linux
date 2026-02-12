use crate::device::ElgatoDevice;
use std::time::Duration;

// 4K S (HID) Control Request parameters
const HID_REQUEST_TYPE_OUT: u8 = 0x21; // Host-to-device, Class, Interface
const HID_REQUEST_TYPE_IN: u8 = 0xA1;  // Device-to-host, Class, Interface
const HID_REQUEST_SET_REPORT: u8 = 0x09;
const HID_REQUEST_GET_REPORT: u8 = 0x01;
const HID_REPORT_VALUE_OUTPUT: u16 = 0x0206; // Report Type=Output(0x02), Report ID=0x06
const HID_REPORT_VALUE_INPUT: u16 = 0x0106;  // Report Type=Input(0x01), Report ID=0x06
pub const HID_INTERFACE_NUM: u16 = 7;
pub const HID_PACKET_SIZE: usize = 255;
const HID_REPORT_ID: u8 = 0x06;

const TIMEOUT: Duration = Duration::from_secs(1);

/// HID Output/Input Report protocol methods for the 4K S.
///
/// Uses SET_REPORT/GET_REPORT requests on Interface 7 with 255-byte zero-padded packets.
/// Write header format: 06 06 06 55 [cmd bytes...]
/// Read request format: 06 55 [sub_cmd] [data_len] (then GET_REPORT to receive response)
impl ElgatoDevice {
    /// Send a single HID output report (must be exactly 255 bytes).
    pub(crate) fn send_hid_packet(&self, packet: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if packet.len() != HID_PACKET_SIZE {
            return Err(format!("HID packet must be exactly {} bytes", HID_PACKET_SIZE).into());
        }

        self.handle.write_control(
            HID_REQUEST_TYPE_OUT,
            HID_REQUEST_SET_REPORT,
            HID_REPORT_VALUE_OUTPUT,
            HID_INTERFACE_NUM,
            packet,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send HID packet: {}", e))?;

        Ok(())
    }

    /// Send a two-packet HID sequence with a 1ms inter-packet delay.
    pub(crate) fn send_hid_two_packet(&self, pkt1: Vec<u8>, pkt2: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        self.send_hid_packet(&pkt1)?;
        std::thread::sleep(Duration::from_millis(1));
        self.send_hid_packet(&pkt2)?;
        Ok(())
    }

    /// Read data from the 4K S by sending a HID read request then GET_REPORT.
    ///
    /// This implements the ReadI2cData protocol from EGAVDeviceSupport:
    ///   1. SET_REPORT with [report_id, cmd, sub_cmd, data_len] to tell device what to send
    ///   2. GET_REPORT (Input) to read back the response
    ///
    /// Returns the raw response bytes (after the report ID byte).
    pub(crate) fn read_hid_data(&self, cmd: u8, sub_cmd: u8, data_len: u8) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Build the read request packet
        let mut request = vec![HID_REPORT_ID, cmd, sub_cmd, data_len];
        request.resize(HID_PACKET_SIZE, 0x00);

        // Send the request via SET_REPORT (Output)
        self.handle.write_control(
            HID_REQUEST_TYPE_OUT,
            HID_REQUEST_SET_REPORT,
            HID_REPORT_VALUE_OUTPUT,
            HID_INTERFACE_NUM,
            &request,
            TIMEOUT,
        ).map_err(|e| format!("Failed to send HID read request: {}", e))?;

        // Small delay for device to prepare response
        std::thread::sleep(Duration::from_millis(10));

        // Read back via GET_REPORT (Input)
        let mut buf = vec![0u8; HID_PACKET_SIZE];
        buf[0] = HID_REPORT_ID; // Report ID must be set in buffer for GET_REPORT

        let len = self.handle.read_control(
            HID_REQUEST_TYPE_IN,
            HID_REQUEST_GET_REPORT,
            HID_REPORT_VALUE_INPUT,
            HID_INTERFACE_NUM,
            &mut buf,
            TIMEOUT,
        ).map_err(|e| format!("Failed to read HID response: {}", e))?;

        // Return data after report ID byte
        if len > 1 {
            Ok(buf[1..len].to_vec())
        } else {
            Ok(vec![])
        }
    }
}
