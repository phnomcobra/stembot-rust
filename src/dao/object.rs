use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};

use crate::dao::document::Document;

/// A handle to a single persisted object in a collection.
/// `T` must be serialisable, deserialisable, and have a `Default` for new
/// object initialisation.
pub struct Object<T: Serialize + DeserializeOwned + Default> {
    pub objuuid: String,
    pub coluuid: String,
    /// The in-memory value.  Mutate this field freely; call `commit` to persist.
    pub object: T,
    document: Document,
}

impl<T: Serialize + DeserializeOwned + Default> Object<T> {
    /// Load an existing object, or create a new one (with default `T` values)
    /// if `objuuid` is not yet stored.
    pub fn new(coluuid: &str, objuuid: &str, document: Document) -> Result<Self> {
        let value = match document.get_object_value(objuuid) {
            Ok(v) => v,
            Err(_) => {
                let default_val = serde_json::to_value(&T::default())?;
                document.create_object_with_value(coluuid, objuuid, &default_val)?;
                document.get_object_value(objuuid)?
            }
        };
        let object: T = serde_json::from_value(value)?;
        Ok(Self {
            objuuid: objuuid.to_string(),
            coluuid: coluuid.to_string(),
            object,
            document,
        })
    }

    /// Persist the current state of `self.object`.
    pub fn commit(&self) -> Result<()> {
        let value = serde_json::to_value(&self.object)?;
        self.document
            .commit_object(&self.coluuid, &self.objuuid, &value)
    }

    /// Delete this object from the database.
    pub fn destroy(self) -> Result<()> {
        self.document.delete_object(&self.objuuid)
    }
}
