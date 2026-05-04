//! HTTP client for secure inter-agent communication.
//!
//! Mirrors Python's `stembot/executor/agent.py`.
//!
//! ## Wire format
//! ```text
//! POST {url}
//! Content-Type: application/octet-stream
//! Nonce: base64(16-byte AES-EAX nonce)
//! Tag:   base64(16-byte AES-EAX MAC tag)
//! Body:  base64(ciphertext)
//! ```
//! All messages are AES-256-EAX encrypted using the 32-byte key derived from
//! `Config::secret_digest`.  `send_network_message` sets `isrc` to the local
//! agent UUID before encrypting.

use anyhow::{anyhow, Result};
use aes::Aes256;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use eax::Eax;
use eax::aead::{Aead, AeadCore, KeyInit};
use rand::rngs::OsRng;

use crate::models::config::Config;
use crate::models::control::ControlFormVariant;
use crate::models::network::NetworkMessage;

type Aes256Eax = Eax<Aes256>;

// ── Crypto primitives ─────────────────────────────────────────────────────────

/// Encrypt `plaintext` with AES-256-EAX using `key`.
///
/// Returns `(nonce, tag, ciphertext)` — all three are needed to decrypt.
/// The nonce is 16 random bytes; the MAC tag is 16 bytes.
pub(crate) fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<([u8; 16], Vec<u8>, Vec<u8>)> {
    let cipher = Aes256Eax::new_from_slice(key)
        .map_err(|e| anyhow!("bad key length: {e:?}"))?;

    let nonce = Aes256Eax::generate_nonce(&mut OsRng);

    // RustCrypto AEAD trait appends the 16-byte MAC tag to the ciphertext.
    let ct_and_tag = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow!("encryption failed: {e:?}"))?;

    let (ct, tag) = ct_and_tag.split_at(ct_and_tag.len() - 16);

    let mut nonce_arr = [0u8; 16];
    nonce_arr.copy_from_slice(&nonce);

    Ok((nonce_arr, tag.to_vec(), ct.to_vec()))
}

/// Decrypt `ciphertext` with AES-256-EAX using `key`, `nonce`, and `tag`.
///
/// Returns the original plaintext, or an error if MAC verification fails.
pub(crate) fn decrypt(
    key: &[u8; 32],
    nonce: &[u8],
    tag: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    let cipher = Aes256Eax::new_from_slice(key)
        .map_err(|e| anyhow!("bad key length: {e:?}"))?;

    // Reconstruct the `ciphertext || tag` layout expected by the AEAD trait.
    let mut ct_with_tag = ciphertext.to_vec();
    ct_with_tag.extend_from_slice(tag);

    let nonce_ga = eax::aead::generic_array::GenericArray::from_slice(nonce);
    cipher
        .decrypt(nonce_ga, ct_with_tag.as_slice())
        .map_err(|_| anyhow!("decryption or MAC verification failed"))
}

// ── AgentClient ───────────────────────────────────────────────────────────────

/// HTTP client for sending encrypted messages to a remote agent.
///
/// Mirrors Python's `AgentClient`.
pub struct AgentClient {
    /// Target URL (e.g., `http://agent:8080/control`).
    pub url: String,
    key: [u8; 32],
    agtuuid: String,
    client: reqwest::Client,
}

impl AgentClient {
    /// Create a client using the current node's configured key and agtuuid.
    pub fn new(url: String) -> Self {
        let config = Config::load();
        Self::with_credentials(url, config.key(), config.agtuuid)
    }

    /// Create a client with explicit credentials (useful in tests).
    pub fn with_credentials(url: String, key: [u8; 32], agtuuid: String) -> Self {
        Self {
            url,
            key,
            agtuuid,
            client: reqwest::Client::new(),
        }
    }

    /// Send a control form and receive a typed response.
    ///
    /// Mirrors `send_control_form(form)`.
    pub async fn send_control_form(
        &self,
        form: ControlFormVariant,
    ) -> Result<ControlFormVariant> {
        let plaintext = serde_json::to_vec(&form)?;
        let (nonce, tag, ct) = encrypt(&self.key, &plaintext)?;

        let response = self
            .client
            .post(&self.url)
            .header("Nonce", B64.encode(nonce))
            .header("Tag", B64.encode(&tag))
            .header("Content-Type", "application/octet-stream")
            .body(B64.encode(&ct))
            .send()
            .await?;

        response.error_for_status_ref()?;
        let plain = self.decrypt_response(response).await?;
        Ok(serde_json::from_slice(&plain)?)
    }

    /// Send a network message and receive a response.
    ///
    /// Sets `isrc` to this agent's UUID before encrypting.
    /// Mirrors `send_network_message(message)`.
    pub async fn send_network_message(
        &self,
        mut msg: NetworkMessage,
    ) -> Result<NetworkMessage> {
        set_isrc(&mut msg, self.agtuuid.clone());

        let plaintext = serde_json::to_vec(&msg)?;
        let (nonce, tag, ct) = encrypt(&self.key, &plaintext)?;

        let response = self
            .client
            .post(&self.url)
            .header("Nonce", B64.encode(nonce))
            .header("Tag", B64.encode(&tag))
            .header("Content-Type", "application/octet-stream")
            .body(B64.encode(&ct))
            .send()
            .await?;

        response.error_for_status_ref()?;
        let plain = self.decrypt_response(response).await?;
        Ok(serde_json::from_slice(&plain)?)
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    async fn decrypt_response(&self, resp: reqwest::Response) -> Result<Vec<u8>> {
        let nonce_b64 = resp
            .headers()
            .get("Nonce")
            .ok_or_else(|| anyhow!("response missing Nonce header"))?
            .to_str()?
            .to_string();
        let tag_b64 = resp
            .headers()
            .get("Tag")
            .ok_or_else(|| anyhow!("response missing Tag header"))?
            .to_str()?
            .to_string();

        let nonce = B64.decode(&nonce_b64)?;
        let tag = B64.decode(&tag_b64)?;
        let body = B64.decode(resp.bytes().await?)?;

        decrypt(&self.key, &nonce, &tag, &body)
    }
}

/// Set the `isrc` field on any [`NetworkMessageVariant`] that carries one.
fn set_isrc(msg: &mut NetworkMessage, isrc: String) {
    use NetworkMessage::*;
    match msg {
        Ping(m)                => m.isrc = Some(isrc),
        MessagesRequest(m)     => m.isrc = Some(isrc),
        MessagesResponse(m)    => m.isrc = Some(isrc),
        Acknowledgement(m)     => m.isrc = Some(isrc),
        Advertisement(m)       => m.isrc = Some(isrc),
        TicketTraceResponse(m) => m.isrc = Some(isrc),
        TicketRequest(m)       => m.isrc = Some(isrc),
        TicketResponse(m)      => m.isrc = Some(isrc),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::control::GetConfig;
    use crate::models::network::Ping;

    // ── Canonical fixtures — must match Python's test_agent.py ────────────────

    const TEST_AGTUUID: &str = "test-agent-id-1";

    /// SHA-256 of b"stembot-test-key" as a 32-byte AES-256 key.
    fn test_key() -> [u8; 32] {
        let hex = sha256::digest("stembot-test-key");
        let bytes = hex::decode(hex).expect("valid hex");
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[..32]);
        key
    }

    const EXPECTED_GET_CONFIG_JSON: &str =
        r#"{"type":"get_config","error":null,"objuuid":null,"coluuid":null,"config":null}"#;

    const EXPECTED_PING_JSON: &str = concat!(
        r#"{"type":"ping","dest":null,"src":"test-agent-id-1","isrc":"test-agent-id-1","#,
        r#""timestamp":1000.0,"objuuid":null,"coluuid":null}"#,
    );

    // Encrypt bytes and return (nonce, tag, ciphertext) — for building mock responses.
    fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> ([u8; 16], Vec<u8>, Vec<u8>) {
        encrypt(key, plaintext).expect("test encryption should succeed")
    }

    // ── Crypto unit tests ─────────────────────────────────────────────────────

    #[test]
    fn test_encrypt_nonce_is_16_bytes() {
        let key = test_key();
        let (nonce, _tag, _ct) = encrypt(&key, b"hello").unwrap();
        assert_eq!(nonce.len(), 16);
    }

    #[test]
    fn test_encrypt_tag_is_16_bytes() {
        let key = test_key();
        let (_nonce, tag, _ct) = encrypt(&key, b"hello").unwrap();
        assert_eq!(tag.len(), 16);
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = b"hello, stembot!";
        let (nonce, tag, ct) = encrypt(&key, plaintext).unwrap();
        let recovered = decrypt(&key, &nonce, &tag, &ct).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn test_decrypt_fails_with_wrong_tag() {
        let key = test_key();
        let (nonce, mut tag, ct) = encrypt(&key, b"hello").unwrap();
        tag[0] ^= 0xFF; // corrupt the tag
        assert!(decrypt(&key, &nonce, &tag, &ct).is_err());
    }

    #[test]
    fn test_decrypt_fails_with_wrong_key() {
        let key = test_key();
        let (nonce, tag, ct) = encrypt(&key, b"hello").unwrap();
        let mut wrong_key = key;
        wrong_key[0] ^= 0xFF;
        assert!(decrypt(&wrong_key, &nonce, &tag, &ct).is_err());
    }

    // ── set_isrc ──────────────────────────────────────────────────────────────

    #[test]
    fn test_set_isrc_ping() {
        let mut msg = NetworkMessage::Ping(Ping::default());
        set_isrc(&mut msg, TEST_AGTUUID.to_string());
        if let NetworkMessage::Ping(p) = msg {
            assert_eq!(p.isrc.as_deref(), Some(TEST_AGTUUID));
        } else {
            panic!("wrong variant");
        }
    }

    // ── send_control_form integration tests ───────────────────────────────────

    async fn make_encrypted_response(
        key: &[u8; 32],
        plaintext: &[u8],
    ) -> ([u8; 16], Vec<u8>, Vec<u8>) {
        encrypt_bytes(key, plaintext)
    }

    #[tokio::test]
    async fn test_send_control_form_nonce_decodes_to_16_bytes() {
        let key = test_key();
        let (r_nonce, r_tag, r_ct) =
            make_encrypted_response(&key, EXPECTED_GET_CONFIG_JSON.as_bytes()).await;

        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/control")
            .with_status(200)
            .with_header("Nonce", &B64.encode(r_nonce))
            .with_header("Tag", &B64.encode(&r_tag))
            .with_body(B64.encode(&r_ct))
            .create_async()
            .await;

        let client = AgentClient::with_credentials(
            format!("{}/control", server.url()),
            key,
            TEST_AGTUUID.to_string(),
        );

        let form = ControlFormVariant::GetConfig(GetConfig::default());
        let result = client.send_control_form(form).await.unwrap();

        // Response should deserialise into GetConfig
        assert!(matches!(result, ControlFormVariant::GetConfig(_)));
    }

    #[tokio::test]
    async fn test_send_control_form_returns_get_config() {
        let key = test_key();
        let (r_nonce, r_tag, r_ct) =
            make_encrypted_response(&key, EXPECTED_GET_CONFIG_JSON.as_bytes()).await;

        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/control")
            .with_status(200)
            .with_header("Nonce", &B64.encode(r_nonce))
            .with_header("Tag", &B64.encode(&r_tag))
            .with_body(B64.encode(&r_ct))
            .create_async()
            .await;

        let client = AgentClient::with_credentials(
            format!("{}/control", server.url()),
            key,
            TEST_AGTUUID.to_string(),
        );

        let form = ControlFormVariant::GetConfig(GetConfig::default());
        let result = client.send_control_form(form).await.unwrap();

        assert!(matches!(result, ControlFormVariant::GetConfig(_)));
    }

    #[tokio::test]
    async fn test_send_control_form_encrypt_decrypt_roundtrip() {
        let key = test_key();
        let (r_nonce, r_tag, r_ct) =
            make_encrypted_response(&key, EXPECTED_GET_CONFIG_JSON.as_bytes()).await;

        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/control")
            .with_status(200)
            .with_header("Nonce", &B64.encode(r_nonce))
            .with_header("Tag", &B64.encode(&r_tag))
            .with_body(B64.encode(&r_ct))
            .create_async()
            .await;

        let client = AgentClient::with_credentials(
            format!("{}/control", server.url()),
            key,
            TEST_AGTUUID.to_string(),
        );

        let form = ControlFormVariant::GetConfig(GetConfig::default());
        let result = client.send_control_form(form).await;
        assert!(result.is_ok());
    }

    // ── send_network_message integration tests ────────────────────────────────

    #[tokio::test]
    async fn test_send_network_message_sets_isrc() {
        let key = test_key();
        let (r_nonce, r_tag, r_ct) =
            make_encrypted_response(&key, EXPECTED_PING_JSON.as_bytes()).await;

        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/mpi")
            .with_status(200)
            .with_header("Nonce", &B64.encode(r_nonce))
            .with_header("Tag", &B64.encode(&r_tag))
            .with_body(B64.encode(&r_ct))
            .create_async()
            .await;

        let client = AgentClient::with_credentials(
            format!("{}/mpi", server.url()),
            key,
            TEST_AGTUUID.to_string(),
        );

        // Build Ping with known fixed values
        let msg = NetworkMessage::Ping(Ping {
            src:       TEST_AGTUUID.to_string(),
            dest:      None,
            isrc:      None, // must be set by send_network_message
            timestamp: Some(1000.0),
            objuuid:   None,
            coluuid:   None,
        });

        let result = client.send_network_message(msg).await.unwrap();

        if let NetworkMessage::Ping(ping) = result {
            assert_eq!(ping.isrc.as_deref(), Some(TEST_AGTUUID));
            assert_eq!(ping.src, TEST_AGTUUID);
            assert_eq!(ping.timestamp, Some(1000.0));
        } else {
            panic!("expected Ping response");
        }
    }

    #[tokio::test]
    async fn test_send_network_message_returns_network_message() {
        let key = test_key();
        let (r_nonce, r_tag, r_ct) =
            make_encrypted_response(&key, EXPECTED_PING_JSON.as_bytes()).await;

        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("POST", "/mpi")
            .with_status(200)
            .with_header("Nonce", &B64.encode(r_nonce))
            .with_header("Tag", &B64.encode(&r_tag))
            .with_body(B64.encode(&r_ct))
            .create_async()
            .await;

        let client = AgentClient::with_credentials(
            format!("{}/mpi", server.url()),
            key,
            TEST_AGTUUID.to_string(),
        );

        let msg = NetworkMessage::Ping(Ping {
            src: TEST_AGTUUID.to_string(),
            ..Ping::default()
        });

        let result = client.send_network_message(msg).await;
        assert!(result.is_ok());
    }

    // ── Canonical plaintext verification ─────────────────────────────────────
    // These tests verify that the serialised JSON sent over the wire matches
    // the canonical Python wire format exactly.

    #[test]
    fn test_get_config_canonical_json() {
        let form = ControlFormVariant::GetConfig(GetConfig::default());
        let got: serde_json::Value = serde_json::to_value(&form).unwrap();
        let exp: serde_json::Value = serde_json::from_str(EXPECTED_GET_CONFIG_JSON).unwrap();
        assert_eq!(got, exp);
    }

    #[test]
    fn test_ping_canonical_json_with_isrc() {
        let mut msg = NetworkMessage::Ping(Ping {
            src:       TEST_AGTUUID.to_string(),
            dest:      None,
            isrc:      None,
            timestamp: Some(1000.0),
            objuuid:   None,
            coluuid:   None,
        });
        set_isrc(&mut msg, TEST_AGTUUID.to_string());

        let json = serde_json::to_string(&msg).unwrap();
        let got: serde_json::Value = serde_json::from_str(&json).unwrap();
        let exp: serde_json::Value = serde_json::from_str(EXPECTED_PING_JSON).unwrap();
        assert_eq!(got, exp);
    }
}
