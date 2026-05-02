//! File I/O operations with compression and integrity verification.
//!
//! Mirrors Python's `stembot/executor/file.py`.
//!
//! ## Wire format
//! - Compression: zlib (level 9, raw deflate via `flate2`)
//! - Encoding:    base64 standard
//! - Checksum:    MD5 hex digest of the **original** (uncompressed) bytes

use std::io::{Read, Write};

use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};

use crate::models::control::{LoadFile, WriteFile};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn md5hex(data: &[u8]) -> String {
    format!("{:x}", md5::compute(data))
}

fn zlib_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(9));
    enc.write_all(data)?;
    Ok(enc.finish()?)
}

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut dec = ZlibDecoder::new(data);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)?;
    Ok(out)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Read a file from disk and populate a [`LoadFile`] form with compressed data.
///
/// Mirrors `load_file_to_form(form: LoadFile) -> LoadFile`.
pub fn load_file_to_form(mut form: LoadFile) -> LoadFile {
    match std::fs::read(&form.path) {
        Ok(data) => {
            form.size   = Some(data.len() as i64);
            form.md5sum = Some(md5hex(&data));
            match zlib_compress(&data) {
                Ok(compressed) => {
                    form.b64zlib = Some(B64.encode(&compressed));
                    form.error   = None;
                }
                Err(e) => {
                    form.b64zlib = None;
                    form.size    = None;
                    form.md5sum  = None;
                    form.error   = Some(e.to_string());
                }
            }
        }
        Err(e) => {
            form.b64zlib = None;
            form.size    = None;
            form.md5sum  = None;
            form.error   = Some(e.to_string());
        }
    }
    form
}

/// Extract and decompress file data from a [`LoadFile`] form.
///
/// Mirrors `load_bytes_from_form(form: LoadFile) -> bytes`.
///
/// # Errors
/// Returns an error if `form.error` is set, if MD5 verification fails, or if
/// decompression fails.
pub fn load_bytes_from_form(form: &LoadFile) -> Result<Vec<u8>> {
    if let Some(ref e) = form.error {
        return Err(anyhow!("form has error: {}", e));
    }
    let b64 = form.b64zlib.as_deref().ok_or_else(|| anyhow!("b64zlib is None"))?;
    let compressed = B64.decode(b64)?;
    let data = zlib_decompress(&compressed)?;
    if let Some(ref expected) = form.md5sum {
        let actual = md5hex(&data);
        if &actual != expected {
            return Err(anyhow!("MD5 mismatch: expected {}, got {}", expected, actual));
        }
    }
    Ok(data)
}

/// Create a [`WriteFile`] form from raw bytes.
///
/// Mirrors `load_form_from_bytes(data: bytes) -> WriteFile`.
pub fn load_form_from_bytes(data: &[u8]) -> WriteFile {
    let compressed = zlib_compress(data).expect("zlib compression failed");
    WriteFile {
        b64zlib: B64.encode(&compressed),
        md5sum:  Some(md5hex(data)),
        size:    Some(data.len() as i64),
        path:    ":memory:".to_string(),
        error:   None,
        objuuid: None,
        coluuid: None,
    }
}

/// Write file data from a [`WriteFile`] form to disk.
///
/// Mirrors `write_file_from_form(form: WriteFile) -> WriteFile`.
pub fn write_file_from_form(mut form: WriteFile) -> WriteFile {
    match write_inner(&form) {
        Ok(()) => {
            form.error   = None;
            form.b64zlib = "".to_string(); // cleared after write, like Python sets None
        }
        Err(e) => {
            form.error = Some(e.to_string());
        }
    }
    form
}

fn write_inner(form: &WriteFile) -> Result<()> {
    let compressed = B64.decode(&form.b64zlib)?;
    let data = zlib_decompress(&compressed)?;

    // Verify pre-write checksum
    if let Some(ref expected) = form.md5sum {
        let actual = md5hex(&data);
        if &actual != expected {
            return Err(anyhow!("MD5 mismatch before write: expected {}, got {}", expected, actual));
        }
    }

    std::fs::write(&form.path, &data)?;

    // Verify post-write checksum
    if let Some(ref expected) = form.md5sum {
        let written = std::fs::read(&form.path)?;
        let actual = md5hex(&written);
        if &actual != expected {
            return Err(anyhow!("MD5 mismatch after write: expected {}, got {}", expected, actual));
        }
    }

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Canonical fixtures — must match Python's test_file.py
    const TEST_DATA:    &[u8] = b"hello, stembot!";
    const TEST_MD5SUM:  &str  = "799be59fc7ab5d892640daa270a9715b";
    const TEST_B64ZLIB: &str  = "eNrLSM3JyddRKC5JzU3KL1EEACz3BYA=";
    const TEST_SIZE:    i64   = 15;

    // ── md5hex ────────────────────────────────────────────────────────────────

    #[test]
    fn test_md5hex_canonical() {
        assert_eq!(md5hex(TEST_DATA), TEST_MD5SUM);
    }

    // ── zlib_compress / zlib_decompress ───────────────────────────────────────

    #[test]
    fn test_compress_canonical() {
        let compressed = zlib_compress(TEST_DATA).unwrap();
        assert_eq!(B64.encode(&compressed), TEST_B64ZLIB);
    }

    #[test]
    fn test_decompress_canonical() {
        let compressed = B64.decode(TEST_B64ZLIB).unwrap();
        let data = zlib_decompress(&compressed).unwrap();
        assert_eq!(data, TEST_DATA);
    }

    #[test]
    fn test_compress_decompress_roundtrip() {
        let compressed = zlib_compress(TEST_DATA).unwrap();
        let recovered  = zlib_decompress(&compressed).unwrap();
        assert_eq!(recovered, TEST_DATA);
    }

    // ── load_form_from_bytes ──────────────────────────────────────────────────

    #[test]
    fn test_load_form_from_bytes_b64zlib() {
        let form = load_form_from_bytes(TEST_DATA);
        assert_eq!(form.b64zlib, TEST_B64ZLIB);
    }

    #[test]
    fn test_load_form_from_bytes_md5sum() {
        let form = load_form_from_bytes(TEST_DATA);
        assert_eq!(form.md5sum.as_deref(), Some(TEST_MD5SUM));
    }

    #[test]
    fn test_load_form_from_bytes_size() {
        let form = load_form_from_bytes(TEST_DATA);
        assert_eq!(form.size, Some(TEST_SIZE));
    }

    #[test]
    fn test_load_form_from_bytes_path() {
        let form = load_form_from_bytes(TEST_DATA);
        assert_eq!(form.path, ":memory:");
    }

    // ── load_bytes_from_form ──────────────────────────────────────────────────

    #[test]
    fn test_load_bytes_from_form_roundtrip() {
        let form = LoadFile {
            path:    ":memory:".to_string(),
            b64zlib: Some(TEST_B64ZLIB.to_string()),
            size:    Some(TEST_SIZE),
            md5sum:  Some(TEST_MD5SUM.to_string()),
            error:   None,
            objuuid: None,
            coluuid: None,
        };
        let data = load_bytes_from_form(&form).unwrap();
        assert_eq!(data, TEST_DATA);
    }

    #[test]
    fn test_load_bytes_from_form_rejects_error_set() {
        let form = LoadFile {
            path:    ":memory:".to_string(),
            b64zlib: Some(TEST_B64ZLIB.to_string()),
            size:    Some(TEST_SIZE),
            md5sum:  Some(TEST_MD5SUM.to_string()),
            error:   Some("previous error".to_string()),
            objuuid: None,
            coluuid: None,
        };
        assert!(load_bytes_from_form(&form).is_err());
    }

    #[test]
    fn test_load_bytes_from_form_rejects_wrong_md5() {
        let form = LoadFile {
            path:    ":memory:".to_string(),
            b64zlib: Some(TEST_B64ZLIB.to_string()),
            size:    Some(TEST_SIZE),
            md5sum:  Some("000000000000000000000000000000000".to_string()),
            error:   None,
            objuuid: None,
            coluuid: None,
        };
        assert!(load_bytes_from_form(&form).is_err());
    }

    // ── load_file_to_form / write_file_from_form ──────────────────────────────

    #[test]
    fn test_load_file_to_form_roundtrip() {
        let dir  = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.bin");
        std::fs::write(&path, TEST_DATA).unwrap();

        let form = load_file_to_form(LoadFile {
            path: path.to_str().unwrap().to_string(),
            ..Default::default()
        });

        assert!(form.error.is_none());
        assert_eq!(form.b64zlib.as_deref(), Some(TEST_B64ZLIB));
        assert_eq!(form.md5sum.as_deref(),  Some(TEST_MD5SUM));
        assert_eq!(form.size,               Some(TEST_SIZE));
    }

    #[test]
    fn test_load_file_to_form_missing_file() {
        let form = load_file_to_form(LoadFile {
            path: "/nonexistent/path/to/file.bin".to_string(),
            ..Default::default()
        });
        assert!(form.error.is_some());
        assert!(form.b64zlib.is_none());
    }

    #[test]
    fn test_write_file_from_form_roundtrip() {
        let dir  = tempfile::tempdir().unwrap();
        let path = dir.path().join("output.bin");

        let form = WriteFile {
            b64zlib: TEST_B64ZLIB.to_string(),
            path:    path.to_str().unwrap().to_string(),
            md5sum:  Some(TEST_MD5SUM.to_string()),
            size:    Some(TEST_SIZE),
            error:   None,
            objuuid: None,
            coluuid: None,
        };

        let result = write_file_from_form(form);
        assert!(result.error.is_none());
        // Python clears b64zlib to None on success; Rust clears to empty string
        assert!(result.b64zlib.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), TEST_DATA);
    }

    #[test]
    fn test_write_file_from_form_bad_path() {
        let form = WriteFile {
            b64zlib: TEST_B64ZLIB.to_string(),
            path:    "/nonexistent/dir/file.bin".to_string(),
            md5sum:  Some(TEST_MD5SUM.to_string()),
            ..Default::default()
        };
        let result = write_file_from_form(form);
        assert!(result.error.is_some());
    }
}
