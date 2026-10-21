use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum KeyType {
    #[serde(rename = "USER")]
    User,
    #[serde(rename = "AUTO")]
    Auto,
    #[serde(rename = "DIST")]
    Dist,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Key {
    pub encrypted_key: Option<String>,
    pub nonce: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub create_time: f64,
    #[serde(default)]
    pub expire_time: f64,
    #[serde(default)]
    pub r#type: KeyType,
    pub agtuuid: Option<String>,
    pub objuuid: Option<String>,
    pub coluuid: Option<String>,
}

impl Key {
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

    pub fn ttl(&self) -> f64 {
        self.expire_time - now()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDistribution {
    pub b64_key: String,
    pub objuuid: String,
    pub ttl: f64,
}

impl KeyDistribution {
    pub fn from_key(key: &[u8; 32], objuuid: String, ttl: f64) -> Self {
        Self {
            b64_key: base64::engine::general_purpose::STANDARD.encode(key),
            objuuid,
            ttl,
        }
    }

    pub fn key(&self) -> Result<[u8; 32], String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&self.b64_key)
            .map_err(|e| e.to_string())?;
        bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| format!("expected a 32-byte key, got {} bytes", bytes.len()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyMetadata {
    pub objuuid: String,
    pub r#type: KeyType,
    pub agtuuid: Option<String>,
    pub ttl: f64,
}
