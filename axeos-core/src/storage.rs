//! Two-slot, versioned settings records. Commit header is written after the payload.
use crate::Config;
pub const SLOT_SIZE: usize = 4096;
pub const HEADER_SIZE: usize = 16;
const MAGIC: &[u8; 4] = b"AXR1";

pub fn encode(
    config: &Config,
    generation: u32,
    buf: &mut [u8; SLOT_SIZE],
) -> Result<usize, &'static str> {
    buf.fill(0xff);
    let size = serde_json_core::to_slice(config, &mut buf[HEADER_SIZE..])
        .map_err(|_| "Configuration trop longue.")?;
    buf[..4].copy_from_slice(MAGIC);
    buf[4..8].copy_from_slice(&generation.to_le_bytes());
    buf[8..10].copy_from_slice(&(size as u16).to_le_bytes());
    buf[10..12].copy_from_slice(&1u16.to_le_bytes());
    let checksum = crc32(&buf[..12], &buf[HEADER_SIZE..HEADER_SIZE + size]);
    buf[12..16].copy_from_slice(&checksum.to_le_bytes());
    Ok((HEADER_SIZE + size + 3) & !3)
}
pub fn decode(buf: &[u8; SLOT_SIZE]) -> Option<(u32, Config)> {
    if &buf[..4] != MAGIC || buf[10..12] != 1u16.to_le_bytes() {
        return None;
    }
    let generation = u32::from_le_bytes(buf[4..8].try_into().ok()?);
    let size = u16::from_le_bytes(buf[8..10].try_into().ok()?) as usize;
    if size > SLOT_SIZE - HEADER_SIZE {
        return None;
    }
    let expected = u32::from_le_bytes(buf[12..16].try_into().ok()?);
    if crc32(&buf[..12], &buf[HEADER_SIZE..HEADER_SIZE + size]) != expected {
        return None;
    }
    let (config, used) = serde_json_core::from_slice(&buf[HEADER_SIZE..HEADER_SIZE + size]).ok()?;
    (used == size).then_some((generation, config))
}
pub fn newer(a: u32, b: u32) -> bool {
    a != b && a.wrapping_sub(b) < (1 << 31)
}
fn crc32(header: &[u8], body: &[u8]) -> u32 {
    let mut crc = !0u32;
    for b in header.iter().chain(body) {
        crc ^= *b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320u32 & (0u32.wrapping_sub(crc & 1)));
        }
    }
    !crc
}
/// Locate only a dedicated, non-overlapping data partition. Never assume stock NVS is ours.
pub fn find_partition(table: &[u8], capacity: u32) -> Option<u32> {
    let mut candidate = None;
    for entry in table.as_chunks::<32>().0 {
        if entry[..2] != [0xaa, 0x50] {
            break;
        }
        if entry[2] == 1 && entry[3] == 0x40 && &entry[12..20] == b"axe_cfg\0" {
            let offset = u32::from_le_bytes(entry[4..8].try_into().ok()?);
            let size = u32::from_le_bytes(entry[8..12].try_into().ok()?);
            if offset < 0x9000
                || offset % 4096 != 0
                || size != 8192
                || offset.checked_add(size)? > capacity
            {
                return None;
            }
            if candidate.replace(offset).is_some() {
                return None;
            }
        }
    }
    let offset = candidate?;
    for entry in table.as_chunks::<32>().0 {
        if entry[..2] != [0xaa, 0x50] {
            break;
        }
        let start = u32::from_le_bytes(entry[4..8].try_into().ok()?);
        let size = u32::from_le_bytes(entry[8..12].try_into().ok()?);
        let is_ours = entry[2] == 1 && entry[3] == 0x40 && &entry[12..20] == b"axe_cfg\0";
        if !is_ours && start < offset + 8192 && start.checked_add(size)? > offset {
            return None;
        }
    }
    Some(offset)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_survive_corruption_and_incomplete_commit() {
        let cfg = Config::defaults("wifi", "password123", 525, 1150);
        let mut slot = [0; SLOT_SIZE];
        let n = encode(&cfg, 42, &mut slot).unwrap();
        assert_eq!(decode(&slot), Some((42, cfg.clone())));
        for idx in [0, 4, 8, 12, 16, n - 4] {
            let mut bad = slot;
            bad[idx] ^= 1;
            assert!(decode(&bad).is_none());
        }
        let mut interrupted = slot;
        interrupted[..16].fill(0xff);
        assert!(decode(&interrupted).is_none());
        assert!(newer(0, u32::MAX));
        assert!(!newer(5, 5));
    }
    #[test]
    fn refuses_stock_nvs_and_overlapping_partitions() {
        let mut table = [0xff; 96];
        table[..2].copy_from_slice(&[0xaa, 0x50]);
        table[2] = 1;
        table[3] = 2;
        table[4..8].copy_from_slice(&0x9000u32.to_le_bytes());
        table[8..12].copy_from_slice(&8192u32.to_le_bytes());
        table[12..20].copy_from_slice(b"nvs\0\0\0\0\0");
        assert_eq!(find_partition(&table, 0x400000), None);
        table[3] = 0x40;
        table[12..20].copy_from_slice(b"axe_cfg\0");
        assert_eq!(find_partition(&table, 0x400000), Some(0x9000));
        table[32..34].copy_from_slice(&[0xaa, 0x50]);
        table[34] = 0;
        table[36..40].copy_from_slice(&0xa000u32.to_le_bytes());
        table[40..44].copy_from_slice(&0x10000u32.to_le_bytes());
        assert_eq!(find_partition(&table, 0x400000), None);
        table[36..40].copy_from_slice(&0x9000u32.to_le_bytes());
        assert_eq!(find_partition(&table, 0x400000), None);
    }
}
