use std::collections::HashMap;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::dao::collection::Collection;

/// Mirrors Python's `KeyValuePair` Pydantic model.
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct KeyValuePair {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub objuuid: Option<String>,
    #[serde(default)]
    pub coluuid: Option<String>,
}

// ── KVStore struct ────────────────────────────────────────────────────────────

/// Persistent key-value store backed by a `Collection<KeyValuePair>`.
///
/// Use `KVStore::new(None)` for the default on-disk store (`kvstore.sqlite`),
/// or supply a connection string (e.g. an in-memory URI for tests).
pub struct KVStore {
    collection: Collection<KeyValuePair>,
}

impl KVStore {
    pub fn new(connection_str: Option<&str>) -> Result<Self> {
        let col = Collection::<KeyValuePair>::new("kvstore", connection_str)?;
        col.create_attribute("name", "/name")?;
        Ok(Self { collection: col })
    }

    pub fn get(&self, name: impl Into<String>, default: Option<Value>) -> Result<Value> {
        let name = name.into();
        let results = self.collection.find(&[("name", &name)])?;
        if let Some(kv) = results.into_iter().next() {
            Ok(kv.object.value)
        } else {
            let default_val = default.unwrap_or(Value::Null);
            self.collection.build_object(KeyValuePair {
                name: name.to_string(),
                value: default_val.clone(),
                ..Default::default()
            })?;
            Ok(default_val)
        }
    }

    pub fn commit(&self, name: impl Into<String>, value: impl Into<Value>) -> Result<()> {
        let name = name.into();
        let results = self.collection.find(&[("name", &name)])?;
        let value = value.into();
        if let Some(mut kv) = results.into_iter().next() {
            kv.object.value = value;
            kv.commit()?;
        } else {
            self.collection.build_object(KeyValuePair {
                name: name.to_string(),
                value,
                ..Default::default()
            })?;
        }
        Ok(())
    }

    pub fn delete(&self, name: impl Into<String>) -> Result<()> {
        let name = name.into();
        self.collection.pop(&[("name", &name)])?;
        Ok(())
    }

    pub fn get_all(&self) -> Result<HashMap<String, Value>> {
        let mut result = HashMap::new();
        for kv in self.collection.find(&[])? {
            result.insert(kv.object.name.clone(), kv.object.value);
        }
        Ok(result)
    }
}

// ── Module-level convenience functions (mirror Python's module API) ───────────

pub fn get(name: impl Into<String>, default: Option<Value>) -> Result<Value> {
    KVStore::new(None)?.get(name, default)
}

pub fn commit(name: impl Into<String>, value: impl Into<Value>) -> Result<()> {
    KVStore::new(None)?.commit(name, value)
}

pub fn delete(name: impl Into<String>) -> Result<()> {
    KVStore::new(None)?.delete(name)
}

pub fn get_all() -> Result<HashMap<String, Value>> {
    KVStore::new(None)?.get_all()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_store() -> KVStore {
        let conn = format!("file:{}?mode=memory&cache=shared", uuid::Uuid::new_v4());
        KVStore::new(Some(&conn)).unwrap()
    }

    #[test]
    fn test_commit_and_get_string() {
        let store = make_store();
        store.commit("key", json!("hello")).unwrap();
        assert_eq!(store.get("key", None).unwrap(), json!("hello"));
    }

    #[test]
    fn test_commit_and_get_int() {
        let store = make_store();
        store.commit("key", json!(42)).unwrap();
        assert_eq!(store.get("key", None).unwrap(), json!(42));
    }

    #[test]
    fn test_commit_and_get_float() {
        let store = make_store();
        store.commit("key", json!(3.14)).unwrap();
        assert_eq!(store.get("key", None).unwrap(), json!(3.14));
    }

    #[test]
    fn test_commit_and_get_null() {
        let store = make_store();
        store.commit("key", Value::Null).unwrap();
        assert_eq!(store.get("key", None).unwrap(), Value::Null);
    }

    #[test]
    fn test_default_value_on_missing_key() {
        let store = make_store();
        let result = store.get("missing", Some(json!("default"))).unwrap();
        assert_eq!(result, json!("default"));
    }

    #[test]
    fn test_default_ignored_when_key_exists() {
        let store = make_store();
        store.commit("key", json!("existing")).unwrap();
        let result = store.get("key", Some(json!("default"))).unwrap();
        assert_eq!(result, json!("existing"));
    }

    #[test]
    fn test_delete() {
        let store = make_store();
        store.commit("key", json!("value")).unwrap();
        store.delete("key").unwrap();
        let result = store.get("key", Some(json!("fallback"))).unwrap();
        assert_eq!(result, json!("fallback"));
    }

    #[test]
    fn test_get_all() {
        let store = make_store();
        store.commit("a", json!(1)).unwrap();
        store.commit("b", json!(2)).unwrap();
        let all = store.get_all().unwrap();
        assert_eq!(all.get("a"), Some(&json!(1)));
        assert_eq!(all.get("b"), Some(&json!(2)));
    }
}
