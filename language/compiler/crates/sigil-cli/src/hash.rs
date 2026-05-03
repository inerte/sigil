pub(crate) fn encode_lower_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

    let bytes = bytes.as_ref();
    let mut rendered = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        rendered.push(HEX_DIGITS[(byte >> 4) as usize] as char);
        rendered.push(HEX_DIGITS[(byte & 0x0f) as usize] as char);
    }

    rendered
}
