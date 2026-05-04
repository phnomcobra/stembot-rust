use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dao::{collection::Collection, document::Document};

pub const CHUNK_SIZE: usize = 65536;

// ── Serde helper: Vec<u8> ↔ base64 string ────────────────────────────────────

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(d)?;
        STANDARD.decode(&encoded).map_err(serde::de::Error::custom)
    }
}

// ── Stored types ──────────────────────────────────────────────────────────────

/// 64 KiB data block.  Binary payload is base64-encoded in JSON.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatastoreChunk {
    #[serde(with = "base64_bytes")]
    pub data: Vec<u8>,
    #[serde(default = "chunk_type")]
    pub r#type: String,
    #[serde(default)]
    pub objuuid: Option<String>,
    #[serde(default)]
    pub coluuid: Option<String>,
}

fn chunk_type() -> String {
    "chunk".to_string()
}

impl Default for DatastoreChunk {
    fn default() -> Self {
        Self {
            data: vec![0u8; CHUNK_SIZE],
            r#type: "chunk".to_string(),
            objuuid: None,
            coluuid: None,
        }
    }
}

/// Ordered list of chunk UUIDs that together form a binary file.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct DatastoreSequence {
    #[serde(default)]
    pub chunks: Vec<String>,
    #[serde(default)]
    pub size: usize,
    #[serde(default = "sequence_type")]
    pub r#type: String,
    #[serde(default)]
    pub objuuid: Option<String>,
    #[serde(default)]
    pub coluuid: Option<String>,
}

fn sequence_type() -> String {
    "sequence".to_string()
}

// ── Internal helpers ──────────────────────────────────────────────────────────

type Datastore = Collection<Value>;

fn new_chunk(datastore: &Datastore) -> Result<(String, DatastoreChunk)> {
    let oid = Document::get_uuid();
    let chunk = DatastoreChunk {
        objuuid: Some(oid.clone()),
        coluuid: Some(datastore.coluuid.clone()),
        ..Default::default()
    };
    datastore.commit_raw(&oid, &serde_json::to_value(&chunk)?)?;
    Ok((oid, chunk))
}

fn new_sequence(datastore: &Datastore, sequuid: Option<&str>) -> Result<(String, DatastoreSequence)> {
    let oid = sequuid.map(|s| s.to_string()).unwrap_or_else(Document::get_uuid);
    let seq = DatastoreSequence {
        r#type: "sequence".to_string(),
        objuuid: Some(oid.clone()),
        coluuid: Some(datastore.coluuid.clone()),
        ..Default::default()
    };
    datastore.commit_raw(&oid, &serde_json::to_value(&seq)?)?;
    Ok((oid, seq))
}

fn get_chunk(datastore: &Datastore, objuuid: &str) -> Result<DatastoreChunk> {
    Ok(serde_json::from_value(datastore.get_raw(objuuid)?)?)
}

fn commit_chunk(datastore: &Datastore, objuuid: &str, chunk: &DatastoreChunk) -> Result<()> {
    datastore.commit_raw(objuuid, &serde_json::to_value(chunk)?)
}

fn get_sequence(datastore: &Datastore, objuuid: &str) -> Result<DatastoreSequence> {
    Ok(serde_json::from_value(datastore.get_raw(objuuid)?)?)
}

fn commit_sequence(datastore: &Datastore, objuuid: &str, seq: &DatastoreSequence) -> Result<()> {
    datastore.commit_raw(objuuid, &serde_json::to_value(seq)?)
}

/// Delete a sequence and all its chunks.
pub fn delete_sequence(datastore: &Datastore, sequuid: &str) -> Result<()> {
    let seq = get_sequence(datastore, sequuid)?;
    for chunk_id in &seq.chunks {
        datastore.delete_raw(chunk_id)?;
    }
    datastore.delete_raw(sequuid)?;
    Ok(())
}

// ── File ──────────────────────────────────────────────────────────────────────

/// A seekable, readable, writable file backed by a datastore collection.
///
/// Mirrors the Python `File` class.  The collection passed in must have a
/// `"type"` attribute indexed on `"/type"` so that sequence objects can be
/// located; call `collection.create_attribute("type", "/type")` before
/// constructing `File`.
pub struct File {
    position: usize,
    chunk_position: usize,
    chunk_index: usize,
    chunk_changed: bool,
    end_of_sequence: bool,
    following_write: bool,
    sequence_uuid: String,
    sequence: DatastoreSequence,
    chunk_uuid: String,
    chunk: DatastoreChunk,
    datastore: Datastore,
}

impl File {
    /// Open an existing sequence or create a new one.
    pub fn new(sequuid: Option<&str>, datastore: Datastore) -> Result<Self> {
        let existing = datastore.find_objuuids(&[("type", "sequence")])?;

        if let Some(sqid) = sequuid {
            if existing.contains(&sqid.to_string()) {
                let sequence = get_sequence(&datastore, sqid)?;
                let chunk_uuid = sequence.chunks[0].clone();
                let chunk = get_chunk(&datastore, &chunk_uuid)?;
                return Ok(Self {
                    position: 0,
                    chunk_position: 0,
                    chunk_index: 0,
                    chunk_changed: false,
                    end_of_sequence: false,
                    following_write: false,
                    sequence_uuid: sqid.to_string(),
                    sequence,
                    chunk_uuid,
                    chunk,
                    datastore,
                });
            }
        }

        let seq_id = sequuid
            .map(|s| s.to_string())
            .unwrap_or_else(Document::get_uuid);
        let (_, mut sequence) = new_sequence(&datastore, Some(&seq_id))?;
        let (chunk_uuid, chunk) = new_chunk(&datastore)?;
        sequence.chunks.push(chunk_uuid.clone());
        commit_sequence(&datastore, &seq_id, &sequence)?;

        Ok(Self {
            position: 0,
            chunk_position: 0,
            chunk_index: 0,
            chunk_changed: false,
            end_of_sequence: false,
            following_write: false,
            sequence_uuid: seq_id,
            sequence,
            chunk_uuid,
            chunk,
            datastore,
        })
    }

    pub fn sequuid(&self) -> &str {
        &self.sequence_uuid
    }

    pub fn delete(self) -> Result<()> {
        delete_sequence(&self.datastore, &self.sequence_uuid)
    }

    pub fn tell(&self) -> usize {
        self.position
    }

    pub fn size(&self) -> usize {
        self.sequence.size
    }

    pub fn close(&mut self) -> Result<()> {
        if self.chunk_changed {
            commit_chunk(&self.datastore, &self.chunk_uuid, &self.chunk)?;
            self.chunk_changed = false;
        }
        commit_sequence(&self.datastore, &self.sequence_uuid, &self.sequence)?;
        Ok(())
    }

    fn load_chunk_at_index(&mut self, index: usize) -> Result<()> {
        if self.chunk_changed {
            commit_chunk(&self.datastore, &self.chunk_uuid, &self.chunk)?;
            self.chunk_changed = false;
        }
        self.chunk_uuid = self.sequence.chunks[index].clone();
        self.chunk = get_chunk(&self.datastore, &self.chunk_uuid)?;
        self.chunk_index = index;
        Ok(())
    }

    pub fn seek(&mut self, seek_position: usize) -> Result<()> {
        if seek_position >= self.sequence.size {
            return Err(anyhow!("Position out of bounds!"));
        }
        let i = seek_position / CHUNK_SIZE;
        if self.chunk_index != i {
            self.load_chunk_at_index(i)?;
        }
        self.chunk_position = seek_position % CHUNK_SIZE;
        self.position = seek_position;
        self.end_of_sequence = false;
        self.following_write = false;
        Ok(())
    }

    /// Read up to `num_bytes` bytes (or the rest of the file if `None`).
    pub fn read(&mut self, num_bytes: Option<usize>) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        if self.end_of_sequence {
            return Ok(buffer);
        }
        let count = match num_bytes {
            None => self.sequence.size.saturating_sub(self.position),
            Some(n) => n,
        };
        for _ in 0..count {
            buffer.push(self.chunk.data[self.chunk_position]);
            let next_pos = self.position + 1;
            if next_pos >= self.sequence.size {
                self.end_of_sequence = true;
                break;
            }
            self.seek(next_pos)?;
        }
        Ok(buffer)
    }

    fn resize(&mut self, num_bytes: usize) -> Result<()> {
        self.sequence.size = num_bytes;
        let num_chunks = num_bytes / CHUNK_SIZE + 1;
        let num_existing = self.sequence.chunks.len();

        if num_existing < num_chunks {
            for _ in num_existing..num_chunks {
                let (chunk_uuid, _) = new_chunk(&self.datastore)?;
                self.sequence.chunks.push(chunk_uuid);
            }
        } else {
            for i in (num_chunks..num_existing).rev() {
                let cid = self.sequence.chunks.remove(i);
                let _ = self.datastore.delete_raw(&cid);
            }
        }
        Ok(())
    }

    /// Write bytes to the file at the current position.
    /// Mirrors the byte-by-byte logic of the Python implementation.
    pub fn write(&mut self, raw_buffer: &[u8]) -> Result<()> {
        if raw_buffer.is_empty() {
            return Ok(());
        }

        // Extend the sequence to fit the incoming data
        if self.following_write && self.sequence.size < self.position + raw_buffer.len() + 1 {
            self.resize(self.position + raw_buffer.len() + 1)?;
            self.seek(self.position + 1)?;
        } else if !self.following_write && self.sequence.size < self.position + raw_buffer.len() {
            self.resize(self.position + raw_buffer.len())?;
        }

        for (i, &byte) in raw_buffer.iter().enumerate() {
            self.chunk.data[self.chunk_position] = byte;
            self.chunk_changed = true;

            if i < raw_buffer.len() - 1 {
                self.seek(self.position + 1)?;
            }
        }

        self.following_write = true;
        Ok(())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    fn make_datastore() -> Datastore {
        let conn = format!("file:{}?mode=memory&cache=shared", uuid::Uuid::new_v4());
        let col = Collection::new(&format!("ds-{}", uuid::Uuid::new_v4()), Some(&conn)).unwrap();
        col.create_attribute("type", "/type").unwrap();
        col
    }

    fn round_trip(data_in: &[u8]) -> Vec<u8> {
        let seq_id = Document::get_uuid_from_str(&format!("test-{}", data_in.len()));
        let ds = make_datastore();
        let mut file = File::new(Some(&seq_id), ds).unwrap();
        file.write(data_in).unwrap();
        if !data_in.is_empty() {
            file.seek(0).unwrap();
        }
        file.read(None).unwrap()
    }

    #[test]
    fn test_file_write_read_16b() {
        let mut data_in = vec![0u8; 16];
        rand::thread_rng().fill_bytes(&mut data_in);
        let data_out = round_trip(&data_in);
        assert_eq!(sha256::digest(data_in.as_slice()), sha256::digest(data_out.as_slice()));
    }

    #[test]
    fn test_file_write_read_chunk_size() {
        let mut data_in = vec![0u8; CHUNK_SIZE];
        rand::thread_rng().fill_bytes(&mut data_in);
        let data_out = round_trip(&data_in);
        assert_eq!(sha256::digest(data_in.as_slice()), sha256::digest(data_out.as_slice()));
    }

    #[test]
    fn test_file_write_read_70k() {
        let mut data_in = vec![0u8; 70 * 1024];
        rand::thread_rng().fill_bytes(&mut data_in);
        let data_out = round_trip(&data_in);
        assert_eq!(sha256::digest(data_in.as_slice()), sha256::digest(data_out.as_slice()));
    }

    #[test]
    fn test_file_write_read_1m() {
        let mut data_in = vec![0u8; 1024 * 1024];
        rand::thread_rng().fill_bytes(&mut data_in);
        let data_out = round_trip(&data_in);
        assert_eq!(sha256::digest(data_in.as_slice()), sha256::digest(data_out.as_slice()));
    }

    #[test]
    fn test_file_write_read_zero() {
        let data_in: &[u8] = b"";
        let data_out = round_trip(data_in);
        assert_eq!(sha256::digest(data_in), sha256::digest(data_out.as_slice()));
    }
}
