//! Cursor pagination.
//!
//! Offsets are wrong for a feed that grows while you read it: a run queued
//! between page one and page two shifts every later row and the reader sees a
//! duplicate or a gap. The cursor here encodes the *position* — the sort key of
//! the last row returned — so the next page continues from a fixed point.
//!
//! The encoding is base64url of `v1:<sort-key>:<id>`, implemented here rather
//! than pulled in: forty lines of base64 is cheaper than a dependency in a
//! browser bundle, and it keeps the crate `no_std`.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{ClientError, Result};

/// Largest page the API will serve. Asking for more is a client bug.
pub const MAX_PAGE_LIMIT: u32 = 100;
pub const DEFAULT_PAGE_LIMIT: u32 = 25;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const CURSOR_VERSION: &str = "v1";

/// An opaque continuation token. Opaque to callers, structured to us — the
/// constructor is the only way to build one, so a hand-written cursor from a
/// URL cannot smuggle in a different sort key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor {
    sort_key: String,
    id: String,
}

impl Cursor {
    /// Build a cursor from the last row of a page.
    pub fn new(sort_key: &str, id: &str) -> Result<Self> {
        if sort_key.is_empty() || id.is_empty() || sort_key.contains(':') || id.contains(':') {
            return Err(ClientError::InvalidCursor);
        }
        Ok(Self {
            sort_key: sort_key.to_owned(),
            id: id.to_owned(),
        })
    }

    pub fn sort_key(&self) -> &str {
        &self.sort_key
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The wire form.
    pub fn encode(&self) -> String {
        let mut raw = String::from(CURSOR_VERSION);
        raw.push(':');
        raw.push_str(&self.sort_key);
        raw.push(':');
        raw.push_str(&self.id);
        base64url_encode(raw.as_bytes())
    }

    /// Parse a cursor the API issued. Anything else fails closed.
    pub fn decode(encoded: &str) -> Result<Self> {
        let bytes = base64url_decode(encoded)?;
        let text = core::str::from_utf8(&bytes).map_err(|_| ClientError::InvalidCursor)?;
        let mut parts = text.splitn(3, ':');
        let version = parts.next().ok_or(ClientError::InvalidCursor)?;
        if version != CURSOR_VERSION {
            return Err(ClientError::InvalidCursor);
        }
        let sort_key = parts.next().ok_or(ClientError::InvalidCursor)?;
        let id = parts.next().ok_or(ClientError::InvalidCursor)?;
        Self::new(sort_key, id)
    }
}

/// A request for one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
    pub after: Option<Cursor>,
}

impl PageRequest {
    pub fn new(limit: u32) -> Result<Self> {
        if limit == 0 || limit > MAX_PAGE_LIMIT {
            return Err(ClientError::PageLimitOutOfRange {
                got: limit,
                max: MAX_PAGE_LIMIT,
            });
        }
        Ok(Self { limit, after: None })
    }

    pub fn after(mut self, cursor: Cursor) -> Self {
        self.after = Some(cursor);
        self
    }

    /// Query-string fragment, ready to append to an endpoint. No leading `?`,
    /// so the caller composes it with whatever else it is sending.
    pub fn to_query(&self) -> String {
        let mut out = String::from("limit=");
        push_u32(&mut out, self.limit);
        if let Some(cursor) = &self.after {
            out.push_str("&after=");
            out.push_str(&cursor.encode());
        }
        out
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            limit: DEFAULT_PAGE_LIMIT,
            after: None,
        }
    }
}

/// One page of results plus the cursor that continues it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<Cursor>,
}

impl<T> Page<T> {
    pub fn new(items: Vec<T>, next: Option<Cursor>) -> Self {
        Self { items, next }
    }

    pub fn is_last(&self) -> bool {
        self.next.is_none()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

fn push_u32(out: &mut String, mut value: u32) {
    if value == 0 {
        out.push('0');
        return;
    }
    let mut digits = [0u8; 10];
    let mut index = digits.len();
    while value > 0 {
        index -= 1;
        digits[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    for digit in &digits[index..] {
        out.push(*digit as char);
    }
}

/// Unpadded base64url, per RFC 4648 §5.
pub fn base64url_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(triple & 0x3f) as usize] as char);
        }
    }
    out
}

/// Unpadded base64url. Rejects padding, whitespace and any character outside
/// the alphabet rather than skipping them.
pub fn base64url_decode(input: &str) -> Result<Vec<u8>> {
    if input.is_empty() {
        return Err(ClientError::InvalidCursor);
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err(ClientError::InvalidCursor),
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    // A well-formed unpadded base64url string leaves fewer than 6 leftover bits,
    // and every one of them must be zero.
    if bits >= 6 || (buffer & ((1 << bits) - 1)) != 0 {
        return Err(ClientError::InvalidCursor);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn base64url_round_trips_every_length_class() {
        for input in [
            &b""[..],
            &b"f"[..],
            &b"fo"[..],
            &b"foo"[..],
            &b"foob"[..],
            &b"fooba"[..],
            &b"foobar"[..],
        ] {
            let encoded = base64url_encode(input);
            if input.is_empty() {
                assert!(encoded.is_empty());
                continue;
            }
            assert_eq!(base64url_decode(&encoded).expect("decode"), input.to_vec());
        }
    }

    #[test]
    fn base64url_uses_the_url_safe_alphabet_and_no_padding() {
        let encoded = base64url_encode(&[0xfb, 0xff, 0xbf]);
        assert!(!encoded.contains('='));
        assert!(!encoded.contains('+'));
        assert!(!encoded.contains('/'));
    }

    #[test]
    fn base64url_rejects_padding_and_foreign_characters() {
        for bad in ["Zm9v=", "Zm 9v", "Zm+v", "Zm/v", ""] {
            assert_eq!(
                base64url_decode(bad),
                Err(ClientError::InvalidCursor),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_cursor_round_trips_through_its_wire_form() {
        let cursor = Cursor::new("2026-09-05T12:00:00Z".replace(':', "-").as_str(), "run-42")
            .expect("valid");
        let decoded = Cursor::decode(&cursor.encode()).expect("decode");
        assert_eq!(decoded, cursor);
        assert_eq!(decoded.id(), "run-42");
    }

    #[test]
    fn a_cursor_refuses_a_sort_key_that_would_break_its_own_encoding() {
        assert_eq!(Cursor::new("a:b", "id"), Err(ClientError::InvalidCursor));
        assert_eq!(Cursor::new("a", "i:d"), Err(ClientError::InvalidCursor));
        assert_eq!(Cursor::new("", "id"), Err(ClientError::InvalidCursor));
    }

    #[test]
    fn a_hand_written_cursor_does_not_decode() {
        assert!(Cursor::decode("not-base64!!").is_err());
        // Well-formed base64url, wrong payload version.
        assert!(Cursor::decode(&base64url_encode(b"v2:key:id")).is_err());
        assert!(Cursor::decode(&base64url_encode(b"onlyonepart")).is_err());
    }

    #[test]
    fn page_requests_are_bounded_at_both_ends() {
        assert!(PageRequest::new(0).is_err());
        assert!(PageRequest::new(MAX_PAGE_LIMIT + 1).is_err());
        assert_eq!(PageRequest::new(1).expect("valid").limit, 1);
        assert_eq!(
            PageRequest::default().limit,
            DEFAULT_PAGE_LIMIT,
            "the default must be inside the range"
        );
        assert!(DEFAULT_PAGE_LIMIT <= MAX_PAGE_LIMIT);
    }

    #[test]
    fn the_query_string_carries_the_cursor_verbatim() {
        let cursor = Cursor::new("2026", "abc").expect("valid");
        let request = PageRequest::new(10).expect("valid").after(cursor.clone());
        let query = request.to_query();
        assert_eq!(query, alloc::format!("limit=10&after={}", cursor.encode()));
        assert!(!PageRequest::new(10)
            .expect("valid")
            .to_query()
            .contains("after"));
    }

    #[test]
    fn a_page_knows_whether_it_is_the_last_one() {
        let full: Page<u8> = Page::new(alloc::vec![1, 2], Cursor::new("k", "i").ok());
        assert!(!full.is_last());
        assert_eq!(full.len(), 2);
        let last: Page<u8> = Page::new(alloc::vec![], None);
        assert!(last.is_last());
        assert!(last.is_empty());
    }

    #[test]
    fn integers_render_without_a_formatter() {
        let mut out = String::new();
        push_u32(&mut out, 0);
        push_u32(&mut out, 100);
        push_u32(&mut out, 4_294_967_295);
        assert_eq!(out, "0".to_string() + "100" + "4294967295");
    }
}
