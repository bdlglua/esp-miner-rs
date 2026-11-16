//! Bounded RFC 6455 handshake and masked client frames; no allocation.
use base64::{engine::general_purpose::STANDARD, Engine};
use sha1::{Digest, Sha1};

pub fn accept(key: &str) -> Option<heapless::String<28>> {
    let mut decoded = [0; 18];
    if STANDARD.decode_slice(key, &mut decoded).ok()? != 16 {
        return None;
    }
    let mut hash = Sha1::new();
    hash.update(key.as_bytes());
    hash.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    let mut encoded = [0; 28];
    STANDARD.encode_slice(hash.finalize(), &mut encoded).ok()?;
    heapless::String::try_from(core::str::from_utf8(&encoded).ok()?).ok()
}

pub struct Frame<'a> {
    pub opcode: u8,
    pub payload: &'a [u8],
    pub consumed: usize,
}
#[derive(Debug, PartialEq)]
pub struct InvalidFrame;
pub fn client_frame(buf: &mut [u8]) -> Result<Option<Frame<'_>>, InvalidFrame> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let opcode = buf[0] & 15;
    if buf[0] & 0xf0 != 0x80 || buf[1] & 0x80 == 0 || !matches!(opcode, 1 | 8 | 9 | 10) {
        return Err(InvalidFrame);
    }
    let n = (buf[1] & 127) as usize;
    // This server accepts only small control/subscription frames.
    if n > 125 {
        return Err(InvalidFrame);
    }
    if buf.len() < n + 6 {
        return Ok(None);
    }
    let mask: [u8; 4] = buf[2..6].try_into().unwrap();
    for (i, b) in buf[6..6 + n].iter_mut().enumerate() {
        *b ^= mask[i % 4];
    }
    Ok(Some(Frame {
        opcode,
        payload: &buf[6..6 + n],
        consumed: n + 6,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc_handshake_and_invalid_keys() {
        assert_eq!(
            accept("dGhlIHNhbXBsZSBub25jZQ==").unwrap(),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
        assert!(accept("invalid").is_none());
    }
    #[test]
    fn fragmented_tcp_input_ping_and_mask_enforcement() {
        let mut data = [0x89, 0x82, 1, 2, 3, 4, b'h' ^ 1, b'i' ^ 2];
        for n in 0..data.len() {
            assert!(client_frame(&mut data[..n]).unwrap().is_none());
        }
        let frame = client_frame(&mut data).unwrap().unwrap();
        assert_eq!(frame.opcode, 9);
        assert_eq!(frame.payload, b"hi");
        assert!(client_frame(&mut [0x89, 0]).is_err());
    }
}
