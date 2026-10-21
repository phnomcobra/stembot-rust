//! Data models for persisted keys and key-exchange messages.
//!
//! These types represent encrypted key records, portable key distributions,
//! and metadata returned by control forms.

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Category of locally stored key.
///
/// The uppercase serialized names are part of the wire and database formats.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum KeyType {
    /// A key derived from a passphrase shared out of band.
    #[serde(rename = "USER")]
    User,
    /// A locally generated random key shared with peers.
    #[serde(rename = "AUTO")]
    Auto,
    /// An automatic key received from a peer.
    #[serde(rename = "DIST")]
    Dist,
    /// An uninitialized or unrecognized key record.
    #[serde(rename = "UNKNOWN")]
    #[default]
    Unknown,
}

impl std::fmt::Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::User => "USER",
            Self::Auto => "AUTO",
            Self::Dist => "DIST",
            Self::Unknown => "UNKNOWN",
        })
    }
}

/// An encrypted key record stored in the local `keys` collection.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Key {
    /// Base64-encoded ciphertext containing the raw key bytes.
    pub encrypted_key: Option<String>,
    /// Base64-encoded nonce used to encrypt the key.
    pub nonce: Option<String>,
    /// Base64-encoded authentication tag for the ciphertext.
    pub tag: Option<String>,
    /// Unix timestamp when the key was created.
    #[serde(default)]
    pub create_time: f64,
    /// Unix timestamp when the key expires.
    #[serde(default)]
    pub expire_time: f64,
    /// Key category.
    #[serde(default)]
    pub r#type: KeyType,
    /// UUID of the peer associated with the key, if peer-specific.
    pub agtuuid: Option<String>,
    /// UUID identifying this key in the collection.
    pub objuuid: Option<String>,
    /// UUID identifying the collection containing this record.
    pub coluuid: Option<String>,
}

impl Key {
    /// Derive the 32-byte wrapping key from this record's identity, metadata, and master key.
    ///
    /// The derivation is part of the persisted-key format; changing it makes
    /// existing encrypted key material unreadable.
    pub fn wrapping_key(&self, master_key: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(self.coluuid.as_deref().unwrap_or("None"));
        hasher.update(self.objuuid.as_deref().unwrap_or("None"));
        hasher.update(self.agtuuid.as_deref().unwrap_or("None"));
        hasher.update(self.r#type.to_string());
        hasher.update((self.create_time as f32).to_ne_bytes());
        hasher.update((self.expire_time as f32).to_ne_bytes());
        hasher.update(master_key);
        hasher.finalize().into()
    }

    /// Return the remaining lifetime in seconds; expired keys have a negative TTL.
    pub fn ttl(&self) -> f64 {
        self.expire_time - now()
    }
}

/// A key and its UUID and lifetime, encoded for distribution to a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDistribution {
    /// Standard Base64 encoding of the 32-byte raw key.
    pub b64_key: String,
    /// UUID identifying the distributed key.
    pub objuuid: String,
    /// Remaining key lifetime in seconds.
    pub ttl: f64,
}

impl KeyDistribution {
    /// Create a distribution from raw key bytes, a UUID, and a lifetime.
    pub fn from_key(key: &[u8; 32], objuuid: String, ttl: f64) -> Self {
        Self {
            b64_key: base64::engine::general_purpose::STANDARD.encode(key),
            objuuid,
            ttl,
        }
    }

    /// Decode the key and require it to contain exactly 32 bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the Base64 value is invalid or does not decode to
    /// exactly 32 bytes.
    pub fn key(&self) -> Result<[u8; 32], String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.b64_key)
            .map_err(|e| e.to_string())?;
        bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| format!("expected a 32-byte key, got {} bytes", bytes.len()))
    }
}

/// Public, non-secret information describing a stored key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    /// UUID identifying the key in the collection.
    pub objuuid: String,
    /// Category of the key.
    pub r#type: KeyType,
    /// Associated peer UUID, if any.
    pub agtuuid: Option<String>,
    /// Remaining key lifetime in seconds; may be negative for an expired key.
    pub ttl: f64,
}
