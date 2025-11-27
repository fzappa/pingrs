/// Calcula o checksum ICMP (RFC 792).
fn checksum(mut data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    while data.len() >= 2 {
        sum = sum.wrapping_add(u16::from_be_bytes([data[0], data[1]]) as u32);
        data = &data[2..];
    }
    if !data.is_empty() {
        sum = sum.wrapping_add((data[0] as u32) << 8);
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Monta um pacote ICMPv4 Echo Request (type=8, code=0).
pub fn build_echo_request(ident: u16, seq: u16, payload: &[u8]) -> Vec<u8> {
    // Cabeçalho ICMP (8 bytes) + payload
    let mut pkt = Vec::with_capacity(8 + payload.len());

    // Type=8 (Echo Request), Code=0, checksum placeholder (2 bytes)
    pkt.extend_from_slice(&[8, 0, 0, 0]);

    // Identifier e Sequence (big-endian)
    pkt.extend_from_slice(&ident.to_be_bytes());
    pkt.extend_from_slice(&seq.to_be_bytes());

    // Payload arbitrário (timestamp, texto, etc.)
    pkt.extend_from_slice(payload);

    // Calcula e escreve o checksum
    let csum = checksum(&pkt);
    pkt[2] = (csum >> 8) as u8;
    pkt[3] = (csum & 0xFF) as u8;

    pkt
}
