use std::marker::PhantomData;

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;

use crate::dao::{document::Document, object::Object};

/// A named, typed, SQLite-backed collection of objects.
///
/// `T` must implement `Serialize + DeserializeOwned + Default`.  Use
/// `serde_json::Value` as `T` for schema-free ("raw") storage.
///
/// Cloning a `Collection` is cheap: the underlying `Document` wraps an
/// `Arc<Mutex<Connection>>`, so all clones share the same SQLite connection
/// and see the same data.  This is the mechanism used by the singleton openers
/// in `collections.rs` to guarantee a single in-memory database per store.
#[derive(Clone)]
pub struct Collection<T: Serialize + DeserializeOwned + Default> {
    pub collection_name: String,
    pub coluuid: String,
    pub connection_str: String,
    pub(crate) document: Document,
    _phantom: PhantomData<T>,
}

impl<T: Serialize + DeserializeOwned + Default> Collection<T> {
    /// Open or create a named collection.
    ///
    /// `connection_str` defaults to `"{name}.sqlite"` if `None`.  Pass
    /// `"file:{uuid}?mode=memory&cache=shared"` for an in-memory database.
    pub fn new(name: &str, connection_str: Option<&str>) -> Result<Self> {
        let conn_str = match connection_str {
            Some(s) => s.to_string(),
            None => format!("{}.sqlite", name),
        };
        let document = Document::new(&conn_str)?;
        let collections = document.list_collections()?;
        let coluuid = match collections.get(name) {
            Some(uuid) => uuid.clone(),
            None => document.create_collection(name)?,
        };
        Ok(Self {
            collection_name: name.to_string(),
            coluuid,
            connection_str: conn_str,
            document,
            _phantom: PhantomData,
        })
    }

    /// Delete the entire collection (and all its objects) from the database.
    pub fn destroy(self) -> Result<()> {
        self.document.delete_collection(&self.coluuid)
    }

    // ── Attributes ────────────────────────────────────────────────────────────

    /// Create or update an indexed attribute.  If the attribute already exists
    /// with a different path, it is deleted and rebuilt.
    pub fn create_attribute(&self, attribute: &str, path: &str) -> Result<()> {
        let attributes = self.document.list_attributes(&self.coluuid)?;
        match attributes.iter().find(|(a, _)| a == attribute) {
            Some((_, p)) if p == path => Ok(()),
            Some(_) => {
                self.document.delete_attribute(&self.coluuid, attribute)?;
                self.document.create_attribute(&self.coluuid, attribute, path)
            }
            None => self.document.create_attribute(&self.coluuid, attribute, path),
        }
    }

    pub fn delete_attribute(&self, attribute: &str) -> Result<()> {
        self.document.delete_attribute(&self.coluuid, attribute)
    }

    // ── Object access ─────────────────────────────────────────────────────────

    /// Return an existing or new `Object<T>`.  A random UUID is generated when
    /// `objuuid` is `None`.
    pub fn get_object(&self, objuuid: Option<&str>) -> Result<Object<T>> {
        let oid = match objuuid {
            Some(id) => id.to_string(),
            None => Document::get_uuid(),
        };
        Object::new(&self.coluuid, &oid, self.document.clone())
    }

    /// Commit `obj` as a new or updated object.  The `objuuid` field inside
    /// the serialised value is used as the key if present; otherwise a fresh
    /// UUID is generated.
    pub fn upsert_object(&self, obj: T) -> Result<Object<T>> {
        let value = serde_json::to_value(&obj)?;
        let objuuid = match value.get("objuuid") {
            Some(Value::String(s)) if !s.is_empty() => s.clone(),
            _ => Document::get_uuid(),
        };
        self.document
            .commit_object(&self.coluuid, &objuuid, &value)?;
        self.get_object(Some(&objuuid))
    }

    /// Convenience alias for `upsert_object`.
    pub fn build_object(&self, obj: T) -> Result<Object<T>> {
        self.upsert_object(obj)
    }

    // ── Search ────────────────────────────────────────────────────────────────

    /// Return all objects matching every `(attribute, expression)` query pair
    /// (intersection).  Empty slice returns all objects.
    ///
    /// Expressions may be plain values (`"red"`) or operator-encoded strings:
    /// `"$gt:2"`, `"$!eq:green"`, `"$startswith:lem"`, `"$regex:^gr.*n$"`, …
    pub fn find(&self, queries: &[(&str, &str)]) -> Result<Vec<Object<T>>> {
        let objuuids = self.document.find_objuuids(&self.coluuid, queries)?;
        let mut objects = Vec::new();
        for objuuid in objuuids {
            match Object::new(&self.coluuid, &objuuid, self.document.clone()) {
                Ok(obj) => objects.push(obj),
                Err(e) => {
                    log::warn!("discarding invalid object {}: {}", objuuid, e);
                    let _ = self.document.delete_object(&objuuid);
                }
            }
        }
        Ok(objects)
    }

    /// Like `find`, but deletes each matched object before returning it.
    pub fn pop(&self, queries: &[(&str, &str)]) -> Result<Vec<Object<T>>> {
        let objuuids = self.document.find_objuuids(&self.coluuid, queries)?;
        let mut objects = Vec::new();
        for objuuid in &objuuids {
            match Object::new(&self.coluuid, objuuid, self.document.clone()) {
                Ok(obj) => objects.push(obj),
                Err(e) => log::warn!("discarding invalid object {}: {}", objuuid, e),
            }
            let _ = self.document.delete_object(objuuid);
        }
        Ok(objects)
    }

    /// Like `find`, but returns at most `limit` objects.
    /// Returns an error if `limit < 1`.
    pub fn find_limited(&self, queries: &[(&str, &str)], limit: usize) -> Result<Vec<Object<T>>> {
        let objuuids = self.document.find_objuuids_limited(&self.coluuid, queries, Some(limit))?;
        let mut objects = Vec::new();
        for objuuid in objuuids {
            match Object::new(&self.coluuid, &objuuid, self.document.clone()) {
                Ok(obj) => objects.push(obj),
                Err(e) => {
                    log::warn!("discarding invalid object {}: {}", objuuid, e);
                    let _ = self.document.delete_object(&objuuid);
                }
            }
        }
        Ok(objects)
    }

    /// Like `pop`, but deletes and returns at most `limit` objects.
    /// Returns an error if `limit < 1`.
    pub fn pop_limited(&self, queries: &[(&str, &str)], limit: usize) -> Result<Vec<Object<T>>> {
        let objuuids = self.document.find_objuuids_limited(&self.coluuid, queries, Some(limit))?;
        let mut objects = Vec::new();
        for objuuid in &objuuids {
            match Object::new(&self.coluuid, objuuid, self.document.clone()) {
                Ok(obj) => objects.push(obj),
                Err(e) => log::warn!("discarding invalid object {}: {}", objuuid, e),
            }
            let _ = self.document.delete_object(objuuid);
        }
        Ok(objects)
    }

    pub fn find_objuuids(&self, queries: &[(&str, &str)]) -> Result<Vec<String>> {
        self.document.find_objuuids(&self.coluuid, queries)
    }

    pub fn list_objuuids(&self) -> Result<Vec<String>> {
        self.document.list_collection_objects(&self.coluuid)
    }

    // ── Raw value access (used by the datastore module) ───────────────────────

    pub fn commit_raw(&self, objuuid: &str, value: &Value) -> Result<()> {
        self.document.commit_object(&self.coluuid, objuuid, value)
    }

    pub fn get_raw(&self, objuuid: &str) -> Result<Value> {
        self.document.get_object_value(objuuid)
    }

    pub fn delete_raw(&self, objuuid: &str) -> Result<()> {
        self.document.delete_object(objuuid)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    /// Mirrors the Python test `Item` / fruit fixture.
    #[derive(Debug, Serialize, Deserialize, Default, Clone)]
    struct Fruit {
        #[serde(default)]
        name: String,
        #[serde(default)]
        size: i64,
        #[serde(default)]
        color: String,
        #[serde(default)]
        objuuid: Option<String>,
        #[serde(default)]
        coluuid: Option<String>,
    }

    fn make_collection() -> Collection<Fruit> {
        let db = format!(
            "file:{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let col: Collection<Fruit> =
            Collection::new(&format!("fruits-{}", uuid::Uuid::new_v4()), Some(&db)).unwrap();
        col.create_attribute("color", "/color").unwrap();
        col.create_attribute("size", "/size").unwrap();
        col.create_attribute("name", "/name").unwrap();

        let items: &[(&str, i64, &str)] = &[
            ("apple", 4, "red"),
            ("lime", 2, "green"),
            ("lemon", 2, "yellow"),
            ("grape", 1, "green"),
        ];
        for &(name, size, color) in items {
            let mut obj = col.get_object(None).unwrap();
            obj.object.name = name.to_string();
            obj.object.size = size;
            obj.object.color = color.to_string();
            obj.commit().unwrap();
        }
        col
    }

    #[test]
    fn test_find_all() {
        let col = make_collection();
        assert_eq!(col.find(&[]).unwrap().len(), 4);
    }

    #[test]
    fn test_find_op_startswith() {
        let col = make_collection();
        assert_eq!(col.find(&[("name", "$startswith:lem")]).unwrap().len(), 1);
    }

    #[test]
    fn test_find_op_endswith() {
        let col = make_collection();
        assert_eq!(col.find(&[("name", "$endswith:mon")]).unwrap().len(), 1);
    }

    #[test]
    fn test_find_op_contains() {
        let col = make_collection();
        assert_eq!(col.find(&[("name", "$contains:emo")]).unwrap().len(), 1);
    }

    #[test]
    fn test_find_op_inside() {
        let col = make_collection();
        assert_eq!(
            col.find(&[("color", "$inside:red and yellow")]).unwrap().len(),
            2
        );
    }

    #[test]
    fn test_find_op_regex() {
        let col = make_collection();
        assert_eq!(col.find(&[("color", "$regex:^gr.*n$")]).unwrap().len(), 2);
    }

    #[test]
    fn test_find_op_gt() {
        let col = make_collection();
        assert_eq!(col.find(&[("size", "$gt:2")]).unwrap().len(), 1);
    }

    #[test]
    fn test_find_op_gte() {
        let col = make_collection();
        assert_eq!(col.find(&[("size", "$gte:2")]).unwrap().len(), 3);
    }

    #[test]
    fn test_find_op_lt() {
        let col = make_collection();
        assert_eq!(col.find(&[("size", "$lt:4")]).unwrap().len(), 3);
    }

    #[test]
    fn test_find_op_lte() {
        let col = make_collection();
        assert_eq!(col.find(&[("size", "$lte:4")]).unwrap().len(), 4);
    }

    #[test]
    fn test_find_op_eq() {
        let col = make_collection();
        assert_eq!(col.find(&[("color", "$eq:red")]).unwrap().len(), 1);
        assert_eq!(col.find(&[("color", "red")]).unwrap().len(), 1);
    }

    #[test]
    fn test_find_op_not_eq() {
        let col = make_collection();
        assert_eq!(col.find(&[("color", "$!eq:red")]).unwrap().len(), 3);
    }

    #[test]
    fn test_find_op_eq_combo() {
        let col = make_collection();
        let items = col.find(&[("color", "green"), ("size", "2")]).unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_find_op_invert_eq_combo() {
        let col = make_collection();
        let items = col
            .find(&[("color", "$!eq:green"), ("size", "$!eq:2")])
            .unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_find_op_gt_lt_combo() {
        let col = make_collection();
        let items = col
            .find(&[("size", "$gt:1"), ("size", "$lt:4")])
            .unwrap();
        assert_eq!(items.len(), 2);
    }

    // ── Reserved attributes ───────────────────────────────────────────────────

    #[test]
    fn test_create_attribute_reserved_name_limit_raises() {
        let col = make_collection();
        let err = col.create_attribute("limit", "/limit").unwrap_err();
        assert!(err.to_string().contains("reserved"), "{err}");
    }

    #[test]
    fn test_delete_attribute_reserved_name_limit_raises() {
        let col = make_collection();
        let err = col.delete_attribute("limit").unwrap_err();
        assert!(err.to_string().contains("reserved"), "{err}");
    }

    // ── Limit parameter ───────────────────────────────────────────────────────

    #[test]
    fn test_find_limit_equal_to_total() {
        let col = make_collection();
        assert_eq!(col.find_limited(&[("size", "$gte:1")], 4).unwrap().len(), 4);
    }

    #[test]
    fn test_find_limit_subset() {
        let col = make_collection();
        assert_eq!(col.find_limited(&[("size", "$gte:1")], 2).unwrap().len(), 2);
    }

    #[test]
    fn test_find_limit_one() {
        let col = make_collection();
        assert_eq!(col.find_limited(&[("size", "$gte:1")], 1).unwrap().len(), 1);
    }

    #[test]
    fn test_find_limit_with_filter() {
        let col = make_collection();
        assert_eq!(col.find_limited(&[("color", "green")], 1).unwrap().len(), 1);
    }

    #[test]
    fn test_find_limit_exceeds_filter_total() {
        let col = make_collection();
        // Only 2 green items; limit=100 returns all 2
        assert_eq!(col.find_limited(&[("color", "green")], 100).unwrap().len(), 2);
    }

    #[test]
    fn test_find_limit_zero_raises() {
        let col = make_collection();
        assert!(col.find_limited(&[("size", "$gte:1")], 0).is_err());
    }

    #[test]
    fn test_find_reserved_attribute_in_queries_skipped() {
        // "limit" as a query attribute is silently skipped; filter on color still applies
        let col = make_collection();
        let items = col.document.find_objuuids_limited(
            &col.coluuid,
            &[("limit", "1"), ("color", "green")],
            None,
        ).unwrap();
        assert_eq!(items.len(), 2);
    }
}
