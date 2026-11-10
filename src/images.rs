
use serde_json::Value;

pub const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageBlock {
    pub mime: String,
    pub data: String,
}

pub fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Some("image/webp");
    }
    None
}

pub fn too_large(len: usize) -> bool {
    len > MAX_BYTES
}

const ALPHABET: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn encode_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn decode_char(byte: u8) -> Option<u32> {
    match byte {
        b'A'..=b'Z' => Some((byte - b'A') as u32),
        b'a'..=b'z' => Some((byte - b'a') as u32 + 26),
        b'0'..=b'9' => Some((byte - b'0') as u32 + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits = 0u32;
    let mut symbols = 0usize;
    let mut padding = false;

    for byte in input.bytes() {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if byte == b'=' {
            padding = true;
            continue;
        }
        if padding {
            return None;
        }
        let value = decode_char(byte)?;
        symbols += 1;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xFF) as u8);
            buffer &= (1 << bits) - 1;
        }
    }

    if symbols % 4 == 1 {
        return None;
    }
    Some(out)
}

pub fn blocks_from_message(message: &Value) -> Vec<ImageBlock> {
    let Some(Value::Array(blocks)) = message.get("content") else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("image"))
        .filter_map(|block| {
            let data = block.get("data").and_then(Value::as_str)?.to_string();
            let mime = block
                .get("mimeType")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    decode_base64(&data)
                        .and_then(|bytes| sniff_mime(&bytes))
                        .map(str::to_string)
                })?;
            Some(ImageBlock { mime, data })
        })
        .collect()
}


