//! Deterministic packet construction for the Elgato 4K X EDID updater.

use sha2::{Digest, Sha256};

use crate::error::ElgatoError;

pub const EDID_SLOT_SIZE: usize = 0x200;
pub const EDID_SLOT_COUNT: usize = 7;
pub const EDID_TABLE_SIZE: usize = 0xe00;
pub const UPDATE_PACKET_SIZE: usize = 0x8a;
pub const UPDATE_BLOCK_SIZE: usize = 0x1000;

const COMMAND_DATA: u16 = 0xee00;
const COMMAND_VERIFY_TRAILER: u16 = 0xee05;
const COMMAND_FINISH_UPDATE: u16 = 0xee06;
const COMMAND_EDID_START: u16 = 0xee03;
const COMMAND_EDID_DONE: u16 = 0xee04;

const UPDATE_TRAILER_HEX: &str = concat!(
    "c015465d163ecb40f0fab39db2625658f62691d72c434d4458b59d2a57caf34f",
    "77316c56f2077560ef568cdd590d58f0ae8c29de61eeaf2ad54a331ed6da79cc",
    "332d95ba2c1d507a1545b37e8e7b73b644f776a2a788b62ecfb68ab227692f84",
    "4b93a388f82a565386e72129e4ea209f12c72c6c8eda7b5dc4e14894449df592",
    "8ce2714d8c15571e76fd587c41218ebe39e94057717a6f44ff6365b8d918946b",
    "b1fec2240af8db22c6d133e3290637f9149a865aa5d0a4d10851c8baf4ebfec7",
    "01b7af8032bb31ff3a1688a695ecab2127a6c4adb731b1f761a0092edbadf779",
    "2123a318402f97b53e780908e5ed2fcf4948f4e1feb159a4b6249a3d7d470ce3"
);

fn hex_bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]))
        .collect()
}

fn hex_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("invalid embedded hex"),
    }
}

fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc = 0u16;
    for byte in data {
        crc ^= u16::from(*byte) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

pub fn validate_edid(edid: &[u8]) -> Result<(), ElgatoError> {
    if edid.is_empty() || edid.len() > EDID_SLOT_SIZE || edid.len() % 128 != 0 {
        return Err(ElgatoError::InvalidEdid(format!(
            "invalid length {}",
            edid.len()
        )));
    }
    if edid[..8] != [0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0] {
        return Err(ElgatoError::InvalidEdid("invalid EDID header".into()));
    }
    if (usize::from(edid[126]) + 1) * 128 != edid.len() {
        return Err(ElgatoError::InvalidEdid(
            "extension count does not match length".into(),
        ));
    }
    for block in edid.chunks_exact(128) {
        if block.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
            return Err(ElgatoError::InvalidEdid("invalid EDID checksum".into()));
        }
    }
    Ok(())
}

/// Packets and the resulting complete flash table for one EDID update.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EdidUploadPlan {
    pub table: Vec<u8>,
    pub entry: Vec<u8>,
    pub verify_trailer: Vec<u8>,
    pub start: Vec<u8>,
    pub data_control: Vec<u8>,
    pub data: Vec<u8>,
    pub done: Vec<u8>,
    pub finalize: Vec<u8>,
}

impl EdidUploadPlan {
    pub fn build(current_table: &[u8], slot: u8, edid: &[u8]) -> Result<Self, ElgatoError> {
        validate_edid(edid)?;
        if current_table.len() != EDID_TABLE_SIZE {
            return Err(ElgatoError::InvalidEdid(
                "flash table must be 0xe00 bytes".into(),
            ));
        }
        if usize::from(slot) >= EDID_SLOT_COUNT {
            return Err(ElgatoError::InvalidEdid("slot must be 0 through 6".into()));
        }
        let mut table = current_table.to_vec();
        let start = usize::from(slot) * EDID_SLOT_SIZE;
        table[start..start + edid.len()].copy_from_slice(edid);

        let table_hash = Sha256::digest(&table);
        let trailer_hash = Sha256::digest(hex_bytes(UPDATE_TRAILER_HEX));
        let data_crc = crc16_ccitt(&table);
        let mut block = [0u8; UPDATE_BLOCK_SIZE];
        block[..table.len()].copy_from_slice(&table);

        Ok(Self {
            table: table.clone(),
            entry: entry_packet(),
            verify_trailer: command_packet(COMMAND_VERIFY_TRAILER, |packet| {
                packet[24..56].copy_from_slice(&trailer_hash);
            }),
            start: command_packet(COMMAND_EDID_START, |packet| {
                packet[24..34].copy_from_slice(b"EDID_Flash");
                packet[40..44].copy_from_slice(&(table.len() as u32).to_le_bytes());
                packet[44..48].copy_from_slice(&(UPDATE_BLOCK_SIZE as u32).to_le_bytes());
                packet[48..80].copy_from_slice(&table_hash);
                // The native Windows packet leaves this field zero. The
                // firmware's logical flash offset is selected internally;
                // inserting 0x00480000 here causes EE03 to be rejected.
                let metadata_crc = crc16_ccitt(&packet[24..86]);
                packet[86..88].copy_from_slice(&metadata_crc.to_le_bytes());
            }),
            data_control: command_packet(COMMAND_DATA, |packet| {
                packet[24..26].copy_from_slice(&(table.len() as u16).to_le_bytes());
                packet[26..28].copy_from_slice(&data_crc.to_le_bytes());
                packet[28..32].copy_from_slice(&0u32.to_le_bytes());
                let metadata_crc = crc16_ccitt(&packet[24..34]);
                packet[34..36].copy_from_slice(&metadata_crc.to_le_bytes());
            }),
            data: data_packet(&block),
            done: command_packet(COMMAND_EDID_DONE, |_| {}),
            finalize: rescue_finalize_packet(),
        })
    }
}

pub(crate) fn rescue_finalize_packet() -> Vec<u8> {
    command_packet(COMMAND_FINISH_UPDATE, |_| {})
}

fn entry_packet() -> Vec<u8> {
    let mut packet = vec![0u8; UPDATE_PACKET_SIZE];
    packet[0..2].copy_from_slice(&0x8005u16.to_le_bytes());
    packet[2] = 1;
    packet[8..20].copy_from_slice(b"enter_rescue");
    let packet_crc = crc16_ccitt(&packet[8..136]);
    packet[136..138].copy_from_slice(&packet_crc.to_le_bytes());
    packet
}

fn command_packet(opcode: u16, fill: impl FnOnce(&mut [u8])) -> Vec<u8> {
    let mut packet = vec![0u8; UPDATE_PACKET_SIZE];
    packet[0..2].copy_from_slice(&0x8005u16.to_le_bytes());
    packet[2] = 1;
    packet[8..15].copy_from_slice(b"upgrade");
    packet[16..18].copy_from_slice(&opcode.to_le_bytes());
    fill(&mut packet);
    let packet_crc = crc16_ccitt(&packet[8..136]);
    packet[136..138].copy_from_slice(&packet_crc.to_le_bytes());
    packet
}

fn data_packet(block: &[u8; UPDATE_BLOCK_SIZE]) -> Vec<u8> {
    let mut packet = vec![0u8; UPDATE_BLOCK_SIZE + 10];
    packet[0] = 3;
    packet[8..8 + UPDATE_BLOCK_SIZE].copy_from_slice(block);
    let block_crc = crc16_ccitt(block);
    packet[8 + UPDATE_BLOCK_SIZE..10 + UPDATE_BLOCK_SIZE].copy_from_slice(&block_crc.to_le_bytes());
    packet
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_edid() -> Vec<u8> {
        valid_edid_blocks(1)
    }

    fn valid_edid_blocks(blocks: usize) -> Vec<u8> {
        let mut edid = vec![0u8; blocks * 128];
        edid[..8].copy_from_slice(&[0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0]);
        edid[126] = (blocks - 1) as u8;
        for block in edid.chunks_exact_mut(128) {
            finish_block(block);
        }
        edid
    }

    fn finish_block(block: &mut [u8]) {
        block[127] = 0u8.wrapping_sub(block[..127].iter().fold(0u8, |s, b| s.wrapping_add(*b)));
    }

    #[test]
    fn accepts_all_supported_edid_lengths() {
        for blocks in 1..=4 {
            assert!(validate_edid(&valid_edid_blocks(blocks)).is_ok());
        }
    }

    #[test]
    fn rejects_invalid_lengths_without_panicking() {
        for size in [0, 1, 7, 127, 129, 255, 513, 640] {
            assert!(validate_edid(&vec![0; size]).is_err());
        }
    }

    #[test]
    fn rejects_bad_header_extension_count_and_each_block_checksum() {
        let mut header = valid_edid();
        header[0] = 1;
        finish_block(&mut header);
        assert!(
            validate_edid(&header)
                .unwrap_err()
                .to_string()
                .contains("header")
        );

        let mut extensions = valid_edid_blocks(2);
        extensions[126] = 0;
        finish_block(&mut extensions[..128]);
        assert!(
            validate_edid(&extensions)
                .unwrap_err()
                .to_string()
                .contains("extension count")
        );

        for block in 0..4 {
            let mut edid = valid_edid_blocks(4);
            edid[block * 128 + 35] ^= 1;
            assert!(
                validate_edid(&edid)
                    .unwrap_err()
                    .to_string()
                    .contains("checksum")
            );
        }
    }

    #[test]
    fn rejects_invalid_slots_and_table_sizes_before_building_packets() {
        let edid = valid_edid();
        for size in [0, EDID_TABLE_SIZE - 1, EDID_TABLE_SIZE + 1, 4096] {
            assert!(EdidUploadPlan::build(&vec![0; size], 0, &edid).is_err());
        }
        for slot in [7, u8::MAX] {
            assert!(EdidUploadPlan::build(&vec![0; EDID_TABLE_SIZE], slot, &edid).is_err());
        }
        assert!(EdidUploadPlan::build(&vec![0; EDID_TABLE_SIZE], 0, &[]).is_err());
    }

    #[test]
    fn captured_enter_rescue_crc_is_reproducible() {
        let mut packet = [0u8; UPDATE_PACKET_SIZE];
        packet[0..2].copy_from_slice(&0x8005u16.to_le_bytes());
        packet[2] = 1;
        packet[8..20].copy_from_slice(b"enter_rescue");
        let packet_crc = crc16_ccitt(&packet[8..136]);
        packet[136..138].copy_from_slice(&packet_crc.to_le_bytes());
        assert_eq!(&packet[136..138], &[0x1e, 0x4f]);
    }

    #[test]
    fn plan_replaces_only_requested_slot_bytes() {
        let table: Vec<u8> = (0..EDID_TABLE_SIZE).map(|i| (i * 29 + 7) as u8).collect();
        for slot in 0..EDID_SLOT_COUNT {
            for blocks in 1..=4 {
                let edid = valid_edid_blocks(blocks);
                let plan = EdidUploadPlan::build(&table, slot as u8, &edid).unwrap();
                let offset = slot * EDID_SLOT_SIZE;
                assert_eq!(&plan.table[..offset], &table[..offset]);
                assert_eq!(&plan.table[offset..offset + edid.len()], edid);
                assert_eq!(
                    &plan.table[offset + edid.len()..],
                    &table[offset + edid.len()..]
                );
            }
        }
    }

    #[test]
    fn packet_sizes_and_crc_are_exact() {
        let plan = EdidUploadPlan::build(&vec![0; EDID_TABLE_SIZE], 0, &valid_edid()).unwrap();
        assert_eq!(plan.entry.len(), UPDATE_PACKET_SIZE);
        assert_eq!(plan.verify_trailer.len(), UPDATE_PACKET_SIZE);
        assert_eq!(plan.start.len(), UPDATE_PACKET_SIZE);
        assert_eq!(plan.data_control.len(), UPDATE_PACKET_SIZE);
        assert_eq!(plan.data.len(), UPDATE_BLOCK_SIZE + 10);
        assert_eq!(plan.done.len(), UPDATE_PACKET_SIZE);
        assert_eq!(plan.finalize.len(), UPDATE_PACKET_SIZE);
        for packet in [
            &plan.entry,
            &plan.verify_trailer,
            &plan.start,
            &plan.data_control,
            &plan.done,
            &plan.finalize,
        ] {
            assert_eq!(
                u16::from_le_bytes(packet[136..138].try_into().unwrap()),
                crc16_ccitt(&packet[8..136])
            );
        }
        assert_eq!(
            u32::from_le_bytes(plan.start[40..44].try_into().unwrap()),
            EDID_TABLE_SIZE as u32
        );
        assert_eq!(&plan.start[48..80], Sha256::digest(&plan.table).as_slice());
        assert_eq!(
            u16::from_le_bytes(plan.data_control[24..26].try_into().unwrap()),
            EDID_TABLE_SIZE as u16
        );
        assert_eq!(
            u32::from_le_bytes(plan.data_control[28..32].try_into().unwrap()),
            0
        );
        assert_eq!(&plan.data[8..8 + EDID_TABLE_SIZE], plan.table);
        assert!(plan.data[8 + EDID_TABLE_SIZE..4104].iter().all(|b| *b == 0));
        assert_eq!(
            u16::from_le_bytes(plan.data_control[26..28].try_into().unwrap()),
            crc16_ccitt(&plan.table)
        );
        assert_eq!(
            u16::from_le_bytes(plan.data[4104..4106].try_into().unwrap()),
            crc16_ccitt(&plan.data[8..4104])
        );
    }

    #[test]
    fn edid_start_preserves_native_zero_flash_field() {
        let plan = EdidUploadPlan::build(&vec![0; EDID_TABLE_SIZE], 0, &valid_edid()).unwrap();
        assert_eq!(&plan.start[80..86], &[0, 0, 0, 0, 0, 0]);
        assert_eq!(
            u16::from_le_bytes(plan.start[86..88].try_into().unwrap()),
            crc16_ccitt(&plan.start[24..86])
        );
    }

    #[test]
    fn packet_order_matches_open_port_windows_edid_flash_branch() {
        let plan = EdidUploadPlan::build(&vec![0; EDID_TABLE_SIZE], 0, &valid_edid()).unwrap();
        assert_eq!(&plan.entry[8..20], b"enter_rescue");
        assert_eq!(
            u16::from_le_bytes(plan.verify_trailer[16..18].try_into().unwrap()),
            COMMAND_VERIFY_TRAILER
        );
        assert_eq!(
            u16::from_le_bytes(plan.start[16..18].try_into().unwrap()),
            COMMAND_EDID_START
        );
        assert_eq!(
            u16::from_le_bytes(plan.done[16..18].try_into().unwrap()),
            COMMAND_EDID_DONE
        );
        assert_eq!(
            u16::from_le_bytes(plan.finalize[16..18].try_into().unwrap()),
            COMMAND_FINISH_UPDATE
        );
    }
}
