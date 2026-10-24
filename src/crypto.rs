//! Local key lifecycle and encrypted key storage.
//!
//! [`KeyManager`] derives user keys, creates random automatic keys, stores
//! distributed peer keys, and encrypts key material before persisting it in
//! the shared `keys` collection. A process-wide manager is available through
//! [`key_manager`]. The master key is read from the platform credential store
//! when available, with a private file fallback.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{anyhow, Context, Result};
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
const KEYRING_SERVICE: &str = "stembot";
const MASTER_KEY_NAME: &str = "master_key";

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Derive the 32-byte user key shared by peers using the same passphrase and key UUID.
///
/// The versioned salt and PBKDF2-HMAC-SHA256 parameters are part of the
/// cross-implementation protocol and must remain compatible with other agents.
///
/// The salt is `stembot:user-key:v1:{objuuid}` and the result is always 32 bytes.
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

/// Manages encrypted user, automatic, and distributed keys in the shared key collection.
pub struct KeyManager {
    master_key: [u8; 32],
    keys: Collection<Key>,
}

impl KeyManager {
    /// Load the master key, open the shared key collection, and seed an automatic key if empty.
    ///
    /// # Errors
    ///
    /// Returns an error if the master key, collection, or initial automatic key
    /// cannot be loaded or created.
    pub fn new() -> Result<Self> {
        let master_key = load_or_create_master_key()?;

        let keys = open_keys()?;

        let manager = Self { master_key, keys };
        if manager.keys.list_objuuids()?.is_empty() {
            manager.create_auto_key()?;
        }
        Ok(manager)
    }

    /// Derive and store a passphrase-based user key for `objuuid`.
    ///
    /// User keys are valid for ten minutes. `agtuuid`, when supplied, limits
    /// the key to the associated peer.
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

    /// Generate and store a random automatic key, returning its object UUID.
    ///
    /// Automatic keys are valid for 90 days.
    pub fn create_auto_key(&self) -> Result<String> {
        let mut raw_key = [0u8; 32];
        OsRng.fill_bytes(&mut raw_key);
        let object = self.keys.get_object(None)?;
        let id = object.objuuid.clone();
        self.store_key(&id, raw_key, KeyType::Auto, None, AUTO_KEY_LIFETIME_SECS)?;
        Ok(id)
    }

    /// Store a received key as a distributed key for the specified peer.
    ///
    /// The supplied TTL must be finite and greater than zero.
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

    /// Decrypt and return a stored key.
    ///
    /// Unknown and expired records are destroyed; records whose authentication
    /// check fails are also destroyed before an error is returned.
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

    /// Select the best key UUID for an optional peer UUID.
    ///
    /// Keys assigned to the peer and unassigned keys are candidates. Distributed
    /// keys take precedence over user keys, which take precedence over automatic
    /// keys; ties prefer the later expiration time.
    /// Returns `None` if no candidate key exists.
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

    /// Build distributions for all unexpired automatic keys.
    ///
    /// Keys that cannot be decrypted are skipped and logged.
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

    /// Store received distributions as keys associated with `agtuuid`.
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

    /// Delete the key identified by `objuuid` if it is a distributed key.
    ///
    /// This is a no-op when the UUID is absent or identifies another key type.
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

    /// Delete all key records whose expiration time has passed.
    pub fn delete_expired_keys(&self) -> Result<()> {
        for key in self.keys.find(&[])? {
            if now() >= key.object.expire_time {
                key.destroy()?;
            }
        }
        Ok(())
    }

    /// Return whether every automatic key is at least 30 days old.
    pub fn all_auto_keys_deprecated(&self) -> Result<bool> {
        let cutoff = now() - KEY_DEPRECATION_SECS;
        Ok(!self
            .keys
            .find(&[])?
            .iter()
            .any(|key| key.object.r#type == KeyType::Auto && key.object.create_time >= cutoff))
    }

    /// Return metadata for records that have an object UUID.
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

    /// Resolve and decrypt the preferred key for a peer URL.
    ///
    /// Loopback URLs use `local_agtuuid`; other URLs are matched to a configured
    /// peer by hostname and effective port.
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
    let entry = match keyring::Entry::new(KEYRING_SERVICE, MASTER_KEY_NAME) {
        Ok(entry) => entry,
        Err(keyring::Error::NoDefaultStore) => {
            log::warn!(
                "OS keyring unavailable ({}); using the owner-only master key file",
                keyring::Entry::store_status()
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "no default store".to_string())
            );
            return load_or_create_file_master_key();
        }
        Err(error) => return Err(error).context("failed to initialize OS keyring"),
    };

    match entry.get_secret() {
        Ok(bytes) => bytes.try_into().map_err(|bytes: Vec<u8>| {
            anyhow!("master key contains {} bytes, expected 32", bytes.len())
        }),
        Err(keyring::Error::NoEntry) => {
            let key = load_or_create_file_master_key()
                .context("failed to load or create the fallback master key")?;

            entry
                .set_secret(&key)
                .context("failed to store master key in OS keyring")?;
            Ok(key)
        }
        Err(error) => Err(error).context("failed to read master key from OS keyring"),
    }
}

fn master_key_path() -> PathBuf {
    PathBuf::from(db_path("kvstore")).with_file_name("stembot-master.key")
}

fn load_or_create_file_master_key() -> Result<[u8; 32]> {
    load_or_create_file_master_key_at(&master_key_path())
}

fn load_or_create_file_master_key_at(path: &Path) -> Result<[u8; 32]> {
    match read_master_key(path) {
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
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(&key)?;
            file.sync_all()?;
            Ok(key)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            read_master_key(path).map_err(Into::into)
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

static KEY_MANAGER: OnceLock<KeyManager> = OnceLock::new();

/// Return the process-wide key manager, initializing it on first use.
///
/// # Panics
///
/// Panics if initialization fails.
pub fn key_manager() -> Result<&'static KeyManager> {
    Ok(KEY_MANAGER.get_or_init(|| KeyManager::new().expect("failed to initialize key manager")))
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
    fn fallback_master_key_is_persisted_with_private_permissions() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("stembot-master.key");

        let first = load_or_create_file_master_key_at(&path).unwrap();
        let second = load_or_create_file_master_key_at(&path).unwrap();
        assert_eq!(first, second);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
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
