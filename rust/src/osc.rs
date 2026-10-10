//! OSC packets in the form VRChat already accepts.
//!
//! Bool messages keep one trailing byte after the type tag. That byte is
//! not part of the OSC 1.0 spec; the desktop app has always sent it.

use std::net::UdpSocket;

use serde_json::Value;

use crate::OSC_PORT;

#[derive(Clone, Debug)]
pub struct OscSender {
    pub host: String,
    pub port: u16,
}

impl Default for OscSender {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: OSC_PORT,
        }
    }
}

impl OscSender {
    pub fn packet(address: &str, value: bool) -> Vec<u8> {
        let flag = if value { b",T" } else { b",F" };
        let extra = if value { 1 } else { 0 };
        let mut out = pad(address.as_bytes());
        out.extend(pad(flag));
        out.push(extra);
        out
    }

    pub fn send_bool(&self, address: &str, value: bool) -> bool {
        let packet = Self::packet(address, value);
        let Ok(sock) = UdpSocket::bind("0.0.0.0:0") else {
            return false;
        };
        sock.send_to(&packet, (self.host.as_str(), self.port))
            .is_ok()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OscTyped {
    Bool(bool),
    Int(i32),
    String(String),
}

impl OscTyped {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::String(_) => "string",
        }
    }
}

pub fn encode_osc_string(address: &str, text: &str) -> Vec<u8> {
    let mut out = pad(address.as_bytes());
    out.extend(pad(b",s"));
    out.extend(pad(text.as_bytes()));
    out
}

pub fn parse_osc_packet(data: &[u8]) -> Option<(String, OscTyped)> {
    if data.is_empty() {
        return None;
    }
    let end = data.iter().position(|byte| *byte == 0)?;
    if end == 0 {
        return None;
    }
    let address = std::str::from_utf8(&data[..end]).ok()?.to_string();
    let index = (end + 4) & !3;
    if index >= data.len() || data[index] != b',' {
        return None;
    }
    let tag_end = data[index..].iter().position(|byte| *byte == 0)? + index;
    let tag = std::str::from_utf8(&data[index + 1..tag_end]).unwrap_or("");
    let cursor = (tag_end + 4) & !3;
    if tag.starts_with('T') {
        return Some((address, OscTyped::Bool(true)));
    }
    if tag.starts_with('F') {
        return Some((address, OscTyped::Bool(false)));
    }
    if tag.starts_with('i') && cursor + 4 <= data.len() {
        let raw = [
            data[cursor],
            data[cursor + 1],
            data[cursor + 2],
            data[cursor + 3],
        ];
        return Some((address, OscTyped::Int(i32::from_be_bytes(raw))));
    }
    if tag.starts_with('s') && cursor < data.len() {
        let text_end = data[cursor..].iter().position(|byte| *byte == 0)? + cursor;
        let text = String::from_utf8_lossy(&data[cursor..text_end]).into_owned();
        return Some((address, OscTyped::String(text)));
    }
    None
}

pub fn osc_from_settings(settings: &Value) -> OscSender {
    let osc = settings.get("osc").and_then(Value::as_object);
    let host = osc
        .and_then(|obj| obj.get("host"))
        .and_then(Value::as_str)
        .filter(|host| !host.is_empty())
        .unwrap_or("127.0.0.1");
    let port = osc
        .and_then(|obj| obj.get("port"))
        .and_then(as_port)
        .unwrap_or(OSC_PORT);
    OscSender {
        host: host.to_string(),
        port,
    }
}

fn as_port(value: &Value) -> Option<u16> {
    let number = value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|n| i64::try_from(n).ok()))
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))?;
    u16::try_from(number).ok().filter(|port| *port != 0)
}

/// OSC 1.0 string: the bytes, a terminating NUL, then NUL bytes up to the
/// next 4-byte boundary. A length that is already a multiple of 4 still
/// needs four NUL bytes (the terminator occupies a whole padding word).
fn pad(raw: &[u8]) -> Vec<u8> {
    let extra = 4 - (raw.len() % 4);
    let mut out = raw.to_vec();
    out.extend(std::iter::repeat_n(0, extra));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expression_bool_bytes_match_the_python_encoder() {
        let down = OscSender::packet("/avatar/parameters/Expression1", true);
        let up = OscSender::packet("/avatar/parameters/Expression1", false);
        assert_eq!(
            down,
            hex("2f6176617461722f706172616d65746572732f45787072657373696f6e3100002c54000001")
        );
        assert_eq!(
            up,
            hex("2f6176617461722f706172616d65746572732f45787072657373696f6e3100002c46000000")
        );
        assert!(down.windows(2).any(|part| part == b",T"));
        assert!(up.windows(2).any(|part| part == b",F"));
        assert_eq!(*down.last().unwrap(), 0x01);
        assert_eq!(*up.last().unwrap(), 0x00);
        let (address, value) = parse_osc_packet(&down).unwrap();
        assert_eq!(address, "/avatar/parameters/Expression1");
        assert_eq!(value, OscTyped::Bool(true));
        assert_eq!(value.kind(), "bool");
    }

    #[test]
    fn short_bool_and_change_string_bytes() {
        assert_eq!(OscSender::packet("/a", true), hex("2f6100002c54000001"));
        let packet = encode_osc_string("/avatar/change", "avtr_smile");
        assert_eq!(
            packet,
            hex("2f6176617461722f6368616e676500002c730000617674725f736d696c650000")
        );
        let (address, value) = parse_osc_packet(&packet).unwrap();
        assert_eq!(address, "/avatar/change");
        assert_eq!(value, OscTyped::String("avtr_smile".into()));
    }

    #[test]
    fn aligned_address_and_type_tags_are_nul_terminated() {
        let address = "/avatar/parameters/GrinA";
        assert_eq!(address.len(), 24);
        assert_eq!(address.len() % 4, 0);

        let down = OscSender::packet(address, true);
        let up = OscSender::packet(address, false);
        assert_eq!(&down[..24], address.as_bytes());
        assert_eq!(&down[24..28], &[0, 0, 0, 0]);
        assert_eq!(&down[28..32], b",T\0\0");
        assert_eq!(down[32], 0x01);
        assert_eq!(&up[..28], &down[..28]);
        assert_eq!(&up[28..32], b",F\0\0");
        assert_eq!(up[32], 0x00);

        let (got, value) = parse_osc_packet(&down).unwrap();
        assert_eq!(got, address);
        assert_eq!(value, OscTyped::Bool(true));
        let (got, value) = parse_osc_packet(&up).unwrap();
        assert_eq!(got, address);
        assert_eq!(value, OscTyped::Bool(false));

        let ordinary = OscSender::packet("/avatar/parameters/Expression1", true);
        assert_eq!(&ordinary[ordinary.len() - 5..], b",T\0\0\x01");
        let ordinary_up = OscSender::packet("/avatar/parameters/Expression1", false);
        assert_eq!(&ordinary_up[ordinary_up.len() - 5..], b",F\0\0\x00");

        let text = encode_osc_string("/avatar/change", "abcd");
        let at = text.windows(4).position(|part| part == b"abcd").unwrap();
        assert_eq!(&text[at..at + 8], b"abcd\0\0\0\0");
        let tags = text.windows(4).position(|part| part == b",s\0\0").unwrap();
        assert!(tags < at);
        let (got, value) = parse_osc_packet(&text).unwrap();
        assert_eq!(got, "/avatar/change");
        assert_eq!(value, OscTyped::String("abcd".into()));
    }

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect()
    }
}
