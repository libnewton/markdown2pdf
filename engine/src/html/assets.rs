//! Host-resolved assets, embedded into the document as `data:` URIs.
//!
//! The engine cannot read files, so both hosts (the Typst package for the CLI,
//! the Web Worker for the browser) resolve every referenced asset themselves
//! and hand the bytes back as one blob plus a `key<TAB>byte-length` manifest.
//! Keys are canonical: `images/<name>`, `remote/<hash>`, `twemoji/<cp>.svg`,
//! `mermaid/<n>.svg`.

use std::collections::HashMap;

pub(crate) struct Assets(HashMap<String, Vec<u8>>);

impl Assets {
    pub(crate) fn decode(manifest: &str, blob: &[u8]) -> Self {
        let mut map = HashMap::new();
        let mut offset = 0usize;
        for line in manifest.lines() {
            let Some((key, len)) = line.split_once('\t') else {
                continue;
            };
            let Ok(len) = len.trim().parse::<usize>() else {
                continue;
            };
            let end = offset.saturating_add(len).min(blob.len());
            if offset >= end && len > 0 {
                break; // truncated blob — drop the rest rather than misalign
            }
            map.insert(key.to_string(), blob[offset..end].to_vec());
            offset = end;
        }
        Self(map)
    }

    #[cfg(test)]
    fn bytes(&self, key: &str) -> Option<&[u8]> {
        self.0.get(key).map(Vec::as_slice)
    }

    /// A `data:` URI for `key`, or `None` when the host could not supply it.
    pub(crate) fn data_uri(&self, key: &str) -> Option<String> {
        let data = self.0.get(key)?;
        if data.is_empty() {
            return None;
        }
        Some(format!("data:{};base64,{}", mime_for(key, data), base64(data)))
    }
}

/// Remote images are keyed by URL hash and carry no extension, so fall back to
/// sniffing the magic bytes rather than guessing wrong.
fn mime_for(key: &str, data: &[u8]) -> &'static str {
    let ext = key.rsplit_once('.').map(|(_, e)| e).unwrap_or_default();
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "svg" => "image/svg+xml",
        "woff2" => "font/woff2",
        _ => sniff(data).unwrap_or("application/octet-stream"),
    }
}

/// Sniff an image type from its leading bytes.
fn sniff(data: &[u8]) -> Option<&'static str> {
    let starts = |sig: &[u8]| data.starts_with(sig);
    if starts(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if starts(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if starts(b"GIF8") {
        Some("image/gif")
    } else if data.len() > 12 && starts(b"RIFF") && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else if data.len() > 12 && &data[4..12] == b"ftypavif" {
        Some("image/avif")
    } else if starts(b"<svg") || starts(b"<?xml") {
        Some("image/svg+xml")
    } else {
        None
    }
}

pub(crate) fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn decode_splits_the_blob_by_manifest_lengths() {
        let a = Assets::decode("images/a.txt\t3\nimages/b.txt\t2\n", b"abcde");
        assert_eq!(a.bytes("images/a.txt"), Some(&b"abc"[..]));
        assert_eq!(a.bytes("images/b.txt"), Some(&b"de"[..]));
        assert_eq!(a.bytes("images/c.txt"), None);
    }

    #[test]
    fn decode_survives_a_truncated_blob() {
        let a = Assets::decode("images/a.txt\t3\nimages/b.txt\t9\n", b"abc");
        assert_eq!(a.bytes("images/a.txt"), Some(&b"abc"[..]));
        assert_eq!(a.data_uri("images/b.txt"), None);
    }

    /// The manifest is host-written, but a bad one must not hand one asset's
    /// bytes out under another's name.
    #[test]
    fn a_malformed_manifest_never_misaligns_the_blob() {
        // A length past the end takes what is left and stops; it does not wrap
        // into the next entry.
        let over = Assets::decode("a\t99\nb\t2\n", b"abcde");
        assert_eq!(over.bytes("a"), Some(&b"abcde"[..]));
        assert_eq!(over.bytes("b"), None);

        // Unparsable and tab-less lines are skipped without consuming bytes.
        let junk = Assets::decode("a\tnope\nno-tab\nb\t2\n", b"xy");
        assert_eq!(junk.bytes("b"), Some(&b"xy"[..]));

        // A repeated key keeps the last slice rather than blending two.
        let dup = Assets::decode("a\t2\na\t3\n", b"xyabc");
        assert_eq!(dup.bytes("a"), Some(&b"abc"[..]));

        // Only the first tab separates; the rest belongs to the key.
        let tabbed = Assets::decode("a\tb\t2\n", b"xy");
        assert_eq!(tabbed.bytes("a"), None);
    }

    #[test]
    fn data_uri_uses_the_key_extension() {
        let a = Assets::decode("images/a.png\t3\n", b"abc");
        assert_eq!(a.data_uri("images/a.png").unwrap(), "data:image/png;base64,YWJj");
    }

    #[test]
    fn data_uri_sniffs_extension_less_remote_keys() {
        let a = Assets::decode("remote/deadbeef\t8\n", b"\x89PNG\r\n\x1a\n");
        assert!(a.data_uri("remote/deadbeef").unwrap().starts_with("data:image/png;base64,"));
    }
}
