pub(crate) fn encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if unreserved(byte) {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex::encode(byte >> 4));
            encoded.push(hex::encode(byte & 0x0f));
        }
    }
    encoded
}

pub(crate) fn decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                let Some(high) = bytes.get(index + 1).and_then(|byte| hex::decode(*byte)) else {
                    return Err("stamp value contains invalid percent escape".to_string());
                };
                let Some(low) = bytes.get(index + 2).and_then(|byte| hex::decode(*byte)) else {
                    return Err("stamp value contains invalid percent escape".to_string());
                };
                decoded.push((high << 4) | low);
                index += 3;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| "stamp value is not valid UTF-8".to_string())
}

fn unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_')
}

mod hex {
    pub(super) fn encode(value: u8) -> char {
        match value {
            0..=9 => (b'0' + value) as char,
            10..=15 => (b'A' + value - 10) as char,
            _ => unreachable!(),
        }
    }

    pub(super) fn decode(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
}
