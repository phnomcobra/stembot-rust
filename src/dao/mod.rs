pub mod collection;
pub mod datastore;
pub mod document;
pub mod kvstore;
pub mod object;

pub use collection::Collection;
pub use datastore::File;
pub use document::{Document, RESERVED_ATTRIBUTES, read_key_at_path};
pub use kvstore::KVStore;
pub use object::Object;

/// Returns the path for a file-backed SQLite database.
///
/// When built with `--features debian`, files are placed in `/var/agt/`.
/// Otherwise they are created in the current working directory.
pub fn db_path(name: &str) -> String {
    #[cfg(feature = "debian")]
    { format!("/var/agt/{}.sqlite", name) }
    #[cfg(not(feature = "debian"))]
    { format!("{}.sqlite", name) }
}
