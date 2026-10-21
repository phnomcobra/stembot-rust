use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{anyhow, Result};
use base64::Engine;
use pbkdf2::pbkdf2_hmac;
use rand::{rngs::OsRng, RngCore};
use sha2::Sha256;

use crate::{
    collections::open_keys,
    dao::{collection::Collection, db_path},
    executor::agent::{decrypt, encrypt},
    models::crypto::{Key, KeyDistribution, KeyMetadata, KeyType},
};

const KDF_ITERATIONS: u32 = 200_000;
const USER_KEY_LIFETIME_SECS: f64 = 600.0;
const AUTO_KEY_LIFETIME_SECS: f64 = 90.0 * 24.0 * 60.0 * 60.0;
const KEY_DEPRECATION_SECS: f64 = 30.0 * 24.0 * 60.0 * 60.0;

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

pub fn derive_user_key(passphrase: &str, objuuid: &str) -> [u8; 32] {
    let salt = format!("stembot:user-key:v1:{objuuid}");
    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(
        passphrase.as_bytes(),
        salt.as_bytes(),
        KDF_ITERATIONS,
        &mut key,
    );
    key
}

pub struct KeyManager {
    master_key: [u8; 32],
    keys: Collection<Key>,
}

impl KeyManager {
    pub fn new() -> Result<Self> {
        let master_key = load_or_create_master_key()?;

        let keys = open_keys()?;

        let manager = Self { master_key, keys };
        if manager.keys.list_objuuids()?.is_empty() {
            manager.create_auto_key()?;
        }
        Ok(manager)
    }

    pub fn create_user_key(
        &self,
        objuuid: &str,
        passphrase: &str,
        agtuuid: Option<&str>,
    ) -> Result<()> {
        let raw_key = derive_user_key(passphrase, objuuid);
        self.store_key(
            objuuid,
            raw_key,
            KeyType::User,
            agtuuid,
            USER_KEY_LIFETIME_SECS,
        )
    }

    pub fn create_auto_key(&self) -> Result<String> {
        let mut raw_key = [0u8; 32];
        OsRng.fill_bytes(&mut raw_key);
        let object = self.keys.get_object(None)?;
        let id = object.objuuid.clone();
        self.store_key(&id, raw_key, KeyType::Auto, None, AUTO_KEY_LIFETIME_SECS)?;
        Ok(id)
    }

    pub fn create_dist_key(
        &self,
        objuuid: &str,
        raw_key: [u8; 32],
        agtuuid: &str,
        ttl: f64,
    ) -> Result<()> {
        if !ttl.is_finite() || ttl <= 0.0 {
            return Err(anyhow!("distributed key TTL must be positive"));
        }
        self.store_key(objuuid, raw_key, KeyType::Dist, Some(agtuuid), ttl)
    }

    fn store_key(
        &self,
        objuuid: &str,
        raw_key: [u8; 32],
        key_type: KeyType,
        agtuuid: Option<&str>,
        ttl: f64,
    ) -> Result<()> {
        let mut key = self.keys.get_object(Some(objuuid))?;
        let create_time = now();
        key.object.create_time = create_time;
        key.object.expire_time = create_time + ttl;
        key.object.r#type = key_type;
        key.object.agtuuid = agtuuid.map(str::to_owned);
        key.object.objuuid = Some(objuuid.to_owned());
        key.object.coluuid = Some(key.coluuid.clone());

        let wrapping_key = key.object.wrapping_key(&self.master_key);
        let (nonce, tag, ciphertext) = encrypt(&wrapping_key, &raw_key)?;
        key.object.encrypted_key =
            Some(base64::engine::general_purpose::STANDARD.encode(ciphertext));
        key.object.nonce = Some(base64::engine::general_purpose::STANDARD.encode(nonce));
        key.object.tag = Some(base64::engine::general_purpose::STANDARD.encode(tag));
        key.commit()
    }

    pub fn decrypt_key(&self, objuuid: &str) -> Result<[u8; 32]> {
        let key = self.keys.get_object(Some(objuuid))?;
        if key.object.r#type == KeyType::Unknown {
            key.destroy()?;
            return Err(anyhow!("unknown key requested: {objuuid}"));
        }
        if now() >= key.object.expire_time {
            key.destroy()?;
            return Err(anyhow!("key '{objuuid}' has expired"));
        }

        let decoded = |value: &Option<String>, name: &str| -> Result<Vec<u8>> {
            let value = value
                .as_deref()
                .ok_or_else(|| anyhow!("key '{objuuid}' is missing {name}"))?;
            Ok(base64::engine::general_purpose::STANDARD.decode(value)?)
        };
        let encrypted_key = decoded(&key.object.encrypted_key, "encrypted_key")?;
        let nonce = decoded(&key.object.nonce, "nonce")?;
        let tag = decoded(&key.object.tag, "tag")?;
        let wrapping_key = key.object.wrapping_key(&self.master_key);
        let raw_key = match decrypt(&wrapping_key, &nonce, &tag, &encrypted_key) {
            Ok(key) => key,
            Err(error) => {
                key.destroy()?;
                return Err(anyhow!(
                    "key integrity check failed for '{objuuid}': {error}"
                ));
            }
        };
        raw_key.try_into().map_err(|bytes: Vec<u8>| {
            anyhow!("key '{objuuid}' has {} bytes, expected 32", bytes.len())
        })
    }

    pub fn resolve_key_objuuid(&self, agtuuid: Option<&str>) -> Result<Option<String>> {
        let mut candidates = self
            .keys
            .find(&[])?
            .into_iter()
            .filter(|key| {
                key.object.agtuuid.as_deref().is_none() || key.object.agtuuid.as_deref() == agtuuid
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            key_priority(a.object.r#type)
                .cmp(&key_priority(b.object.r#type))
                .then_with(|| b.object.expire_time.total_cmp(&a.object.expire_time))
        });
        Ok(candidates
            .first()
            .and_then(|key| key.object.objuuid.clone()))
    }

    pub fn create_key_distributions(&self) -> Result<Vec<KeyDistribution>> {
        let mut distributions = Vec::new();
        for key in self.keys.find(&[])? {
            if key.object.r#type != KeyType::Auto || key.object.expire_time <= now() {
                continue;
            }
            let Some(objuuid) = key.object.objuuid.clone() else {
                continue;
            };
            match self.decrypt_key(&objuuid) {
                Ok(raw_key) => distributions.push(KeyDistribution::from_key(
                    &raw_key,
                    objuuid,
                    key.object.ttl(),
                )),
                Err(error) => log::warn!("could not distribute key {objuuid}: {error}"),
            }
        }
        Ok(distributions)
    }

    pub fn process_key_distributions(
        &self,
        distributions: &[KeyDistribution],
        agtuuid: &str,
    ) -> Result<()> {
        for distribution in distributions {
            self.create_dist_key(
                &distribution.objuuid,
                distribution.key().map_err(|e| anyhow!(e))?,
                agtuuid,
                distribution.ttl,
            )?;
        }
        Ok(())
    }

    pub fn delete_dist_key(&self, objuuid: &str) -> Result<()> {
        if !self.keys.list_objuuids()?.iter().any(|id| id == objuuid) {
            return Ok(());
        }
        let key = self.keys.get_object(Some(objuuid))?;
        if key.object.r#type == KeyType::Dist {
            key.destroy()?;
        }
        Ok(())
    }

    pub fn delete_expired_keys(&self) -> Result<()> {
        for key in self.keys.find(&[])? {
            if now() >= key.object.expire_time {
                key.destroy()?;
            }
        }
        Ok(())
    }

    pub fn all_auto_keys_deprecated(&self) -> Result<bool> {
        let cutoff = now() - KEY_DEPRECATION_SECS;
        Ok(!self
            .keys
            .find(&[])?
            .iter()
            .any(|key| key.object.r#type == KeyType::Auto && key.object.create_time >= cutoff))
    }

    pub fn describe(&self) -> Result<Vec<KeyMetadata>> {
        Ok(self
            .keys
            .find(&[])?
            .into_iter()
            .filter_map(|key| {
                Some(KeyMetadata {
                    objuuid: key.object.objuuid.clone()?,
                    r#type: key.object.r#type,
                    agtuuid: key.object.agtuuid.clone(),
                    ttl: key.object.ttl(),
                })
            })
            .collect())
    }

    pub fn resolve_for_url(&self, url: &str, local_agtuuid: &str) -> Result<(String, [u8; 32])> {
        let target = reqwest::Url::parse(url)?;
        let hostname = target.host_str().unwrap_or_default();
        let agtuuid = if matches!(hostname, "localhost" | "127.0.0.1" | "::1" | "0.0.0.0") {
            Some(local_agtuuid.to_owned())
        } else {
            let peers = crate::collections::open_peers()?.find(&[])?;
            peers.into_iter().find_map(|peer| {
                let peer_url = peer.object.url?;
                let parsed = reqwest::Url::parse(&peer_url).ok()?;
                (parsed.host_str() == target.host_str()
                    && parsed.port_or_known_default() == target.port_or_known_default())
                .then(|| peer.object.agtuuid)
                .flatten()
            })
        };
        let keyuuid = self
            .resolve_key_objuuid(agtuuid.as_deref())?
            .ok_or_else(|| anyhow!("unable to resolve a key UUID for peer URL '{url}'"))?;
        let key = self.decrypt_key(&keyuuid)?;
        Ok((keyuuid, key))
    }
}

fn key_priority(key_type: KeyType) -> u8 {
    match key_type {
        KeyType::Dist => 0,
        KeyType::User => 1,
        KeyType::Auto => 2,
        KeyType::Unknown => 3,
    }
}

fn load_or_create_master_key() -> Result<[u8; 32]> {
    let path = PathBuf::from(db_path("kvstore")).with_file_name("stembot-master.key");
    match read_master_key(&path) {
        Ok(key) => return Ok(key),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(&path) {
        Ok(mut file) => {
            file.write_all(&key)?;
            file.sync_all()?;
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            read_master_key(&path).map_err(Into::into)
        }
        Err(error) => Err(error.into()),
    }
}

fn read_master_key(path: &Path) -> std::io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("master key contains {} bytes, expected 32", bytes.len()),
        )
    })
}

static KEY_MANAGER: OnceLock<std::result::Result<KeyManager, String>> = OnceLock::new();

pub fn key_manager() -> Result<&'static KeyManager> {
    KEY_MANAGER
        .get_or_init(|| KeyManager::new().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|error| anyhow!("{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> KeyManager {
        let connection = format!(
            "file:keys-{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let keys = Collection::new("keys", Some(&connection)).unwrap();
        KeyManager {
            master_key: [0x42; 32],
            keys,
        }
    }

    #[test]
    fn user_key_derivation_matches_python_fixture() {
        assert_eq!(
            hex::encode(derive_user_key(
                "correct horse battery staple",
                "shared-user-key"
            )),
            "035347be3c41eaece303cf4ab8ac565cfb20afd5ac8ee44f7010a1a27d5df8d0"
        );
    }

    #[test]
    fn user_key_is_encrypted_and_resolvable() {
        let manager = test_manager();
        manager
            .create_user_key("shared-key", "shared-passphrase", Some("peer-a"))
            .unwrap();
        assert_eq!(
            manager.resolve_key_objuuid(Some("peer-a")).unwrap(),
            Some("shared-key".to_string())
        );
        assert_eq!(
            manager.decrypt_key("shared-key").unwrap(),
            derive_user_key("shared-passphrase", "shared-key")
        );
        let stored = manager.keys.get_object(Some("shared-key")).unwrap();
        let plaintext = base64::engine::general_purpose::STANDARD
            .encode(derive_user_key("shared-passphrase", "shared-key"));
        assert_ne!(
            stored.object.encrypted_key.as_deref(),
            Some(plaintext.as_str())
        );
    }

    #[test]
    fn auto_keys_distribute_and_import_as_peer_keys() {
        let sender = test_manager();
        let recipient = test_manager();
        let auto_uuid = sender.create_auto_key().unwrap();
        let distributions = sender.create_key_distributions().unwrap();
        assert_eq!(distributions.len(), 1);
        recipient
            .process_key_distributions(&distributions, "peer-a")
            .unwrap();
        assert_eq!(
            recipient.resolve_key_objuuid(Some("peer-a")).unwrap(),
            Some(auto_uuid)
        );
        assert_eq!(
            recipient.decrypt_key(&distributions[0].objuuid).unwrap(),
            distributions[0].key().unwrap()
        );
    }
}
