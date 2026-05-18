use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use regex::Regex;
use rusqlite::{params, Connection, OpenFlags};
use serde_json::Value;
use uuid::Uuid;

pub const DEFAULT_CONNECTION_STR: &str = "default.sqlite";

/// Attribute names that are reserved and cannot be used as index attributes.
/// Mirrors Python's `RESERVED_ATTRIBUTES_NAMES` in `stembot/dao/document.py`.
pub const RESERVED_ATTRIBUTES: &[&str] = &["limit"];

/// Core SQLite-backed storage layer.  Every Collection and Object obtains a
/// clone of a Document so they all share the same `Arc<Mutex<Connection>>`.
#[derive(Clone)]
pub struct Document {
    pub connection_str: String,
    connection: Arc<Mutex<Connection>>,
}

impl Document {
    pub fn new(connection_str: &str) -> Result<Self> {
        let conn = if connection_str.contains(":memory:") || connection_str.contains("mode=memory")
        {
            Connection::open_with_flags(
                connection_str,
                OpenFlags::SQLITE_OPEN_READ_WRITE
                    | OpenFlags::SQLITE_OPEN_CREATE
                    | OpenFlags::SQLITE_OPEN_URI,
            )?
        } else {
            Connection::open(connection_str)?
        };

        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS TBL_COLLECTIONS (
                COLUUID VARCHAR(36),
                NAME    VARCHAR(64) UNIQUE NOT NULL,
                PRIMARY KEY (COLUUID)
             );
             CREATE TABLE IF NOT EXISTS TBL_OBJECTS (
                OBJUUID VARCHAR(36),
                COLUUID VARCHAR(36),
                VALUE   TEXT NOT NULL,
                PRIMARY KEY (OBJUUID),
                FOREIGN KEY (COLUUID) REFERENCES TBL_COLLECTIONS(COLUUID) ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS TBL_ATTRIBUTES (
                COLUUID   VARCHAR(36),
                ATTRIBUTE VARCHAR(64),
                PATH      VARCHAR(64),
                PRIMARY KEY (COLUUID, ATTRIBUTE),
                FOREIGN KEY (COLUUID) REFERENCES TBL_COLLECTIONS(COLUUID) ON DELETE CASCADE
             );
             CREATE TABLE IF NOT EXISTS TBL_INDEX (
                OBJUUID   VARCHAR(36),
                COLUUID   VARCHAR(36),
                ATTRIBUTE VARCHAR(64),
                VALUE     VARCHAR(64),
                PRIMARY KEY (OBJUUID, ATTRIBUTE),
                FOREIGN KEY (OBJUUID) REFERENCES TBL_OBJECTS(OBJUUID) ON DELETE CASCADE,
                FOREIGN KEY (COLUUID, ATTRIBUTE)
                    REFERENCES TBL_ATTRIBUTES(COLUUID, ATTRIBUTE) ON DELETE CASCADE
             );",
        )?;

        Ok(Self {
            connection_str: connection_str.to_string(),
            connection: Arc::new(Mutex::new(conn)),
        })
    }

    // ── UUID helpers ──────────────────────────────────────────────────────────

    pub fn get_uuid() -> String {
        Uuid::new_v4().to_string()
    }

    /// Deterministic UUID derived from a seed string (SHA-256 based), mirroring
    /// `get_uuid_str_from_str` in the Python DAO.
    pub fn get_uuid_from_str(seed: &str) -> String {
        let digest = sha256::digest(seed);
        format!(
            "{}-{}-{}-{}-{}",
            &digest[0..8],
            &digest[8..12],
            &digest[12..16],
            &digest[16..20],
            &digest[20..32]
        )
    }

    // ── Maintenance ───────────────────────────────────────────────────────────

    pub fn vacuum(&self) -> Result<()> {
        self.connection.lock().unwrap().execute_batch(
            "VACUUM; PRAGMA shrink_memory;"
        )?;
        Ok(())
    }

    // ── Object CRUD ───────────────────────────────────────────────────────────

    /// Persist a new object whose initial body is `value` (T's default
    /// serialisation), with `objuuid`/`coluuid` injected.
    pub fn create_object_with_value(
        &self,
        coluuid: &str,
        objuuid: &str,
        value: &Value,
    ) -> Result<()> {
        let mut obj = value.clone();
        if let Some(map) = obj.as_object_mut() {
            map.insert("objuuid".into(), Value::String(objuuid.into()));
            map.insert("coluuid".into(), Value::String(coluuid.into()));
        }
        let conn = self.connection.lock().unwrap();
        conn.execute(
            "INSERT INTO TBL_OBJECTS (COLUUID, OBJUUID, VALUE) VALUES (?1, ?2, ?3);",
            params![coluuid, objuuid, &obj.to_string()],
        )?;
        Ok(())
    }

    /// Upsert an object and rebuild its index entries.
    pub fn commit_object(&self, coluuid: &str, objuuid: &str, object: &Value) -> Result<()> {
        let mut obj = object.clone();
        if let Some(map) = obj.as_object_mut() {
            map.insert("objuuid".into(), Value::String(objuuid.into()));
            map.insert("coluuid".into(), Value::String(coluuid.into()));
        }
        let serialized = obj.to_string();
        let conn = self.connection.lock().unwrap();

        conn.execute(
            "INSERT OR REPLACE INTO TBL_OBJECTS (COLUUID, OBJUUID, VALUE) VALUES (?1, ?2, ?3);",
            params![coluuid, objuuid, &serialized],
        )?;

        // Rebuild index
        let attributes: Vec<(String, String)> = {
            let mut stmt = conn.prepare(
                "SELECT ATTRIBUTE, PATH FROM TBL_ATTRIBUTES WHERE COLUUID = ?1;",
            )?;
            let x: Vec<(String, String)> = stmt
                .query_map(params![coluuid], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            x
        };

        for (attribute, path) in &attributes {
            match read_key_at_path(path, &obj) {
                Some(v) => {
                    let val_str = match &v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO TBL_INDEX \
                         (OBJUUID, COLUUID, ATTRIBUTE, VALUE) VALUES (?1, ?2, ?3, ?4);",
                        params![objuuid, coluuid, attribute, &val_str],
                    );
                }
                None => log::warn!(
                    "error indexing attribute \"{}\" for object \"{}\"",
                    attribute,
                    objuuid
                ),
            }
        }
        Ok(())
    }

    pub fn get_object_value(&self, objuuid: &str) -> Result<Value> {
        let conn = self.connection.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT VALUE FROM TBL_OBJECTS WHERE OBJUUID = ?1;")?;
        match stmt.query_row(params![objuuid], |row| row.get::<_, String>(0)) {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                Err(anyhow!("object not found: {}", objuuid))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn delete_object(&self, objuuid: &str) -> Result<()> {
        let conn = self.connection.lock().unwrap();
        conn.execute(
            "DELETE FROM TBL_OBJECTS WHERE OBJUUID = ?1;",
            params![objuuid],
        )?;
        Ok(())
    }

    // ── Query ─────────────────────────────────────────────────────────────────

    /// Find object UUIDs matching ALL supplied `(attribute, expression)` pairs
    /// (intersection).  Empty `queries` returns every UUID in the collection.
    pub fn find_objuuids(&self, coluuid: &str, queries: &[(&str, &str)]) -> Result<Vec<String>> {
        self.find_objuuids_limited(coluuid, queries, None)
    }

    /// Like `find_objuuids`, but truncates the result to at most `limit` entries.
    /// `limit` must be `>= 1` if `Some`; passing `Some(0)` returns an error.
    /// Reserved attribute names in `queries` are silently skipped.
    pub fn find_objuuids_limited(
        &self,
        coluuid: &str,
        queries: &[(&str, &str)],
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        if let Some(n) = limit {
            if n < 1 {
                return Err(anyhow!("limit must be >= 1, got {}", n));
            }
        }

        // Filter out reserved attribute names from queries
        let effective_queries: Vec<(&str, &str)> = queries
            .iter()
            .copied()
            .filter(|(attr, _)| !RESERVED_ATTRIBUTES.contains(attr))
            .collect();

        if effective_queries.is_empty() {
            let all = self.list_collection_objects(coluuid)?;
            return Ok(match limit {
                Some(n) => all.into_iter().take(n).collect(),
                None => all,
            });
        }

        let conn = self.connection.lock().unwrap();
        let mut objuuid_lists: Vec<Vec<String>> = Vec::new();

        for &(attribute, expression) in &effective_queries {
            let (operator, negation, subject) = parse_expression(expression);

            let ids: Vec<String> = match operator {
                Operator::Eq => {
                    let sql = if negation {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE != ?2 AND COLUUID = ?3;"
                    } else {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE = ?2 AND COLUUID = ?3;"
                    };
                    let mut stmt = conn.prepare(sql)?;
                    let ids: Vec<String> = stmt
                        .query_map(params![attribute, &subject, coluuid], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect();
                    ids
                }
                Operator::Contains => {
                    let pat = format!("%{}%", subject);
                    let sql = if negation {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE NOT LIKE ?2 AND COLUUID = ?3;"
                    } else {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE LIKE ?2 AND COLUUID = ?3;"
                    };
                    let mut stmt = conn.prepare(sql)?;
                    let ids: Vec<String> = stmt
                        .query_map(params![attribute, &pat, coluuid], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect();
                    ids
                }
                Operator::Startswith => {
                    let pat = format!("{}%", subject);
                    let sql = if negation {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE NOT LIKE ?2 AND COLUUID = ?3;"
                    } else {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE LIKE ?2 AND COLUUID = ?3;"
                    };
                    let mut stmt = conn.prepare(sql)?;
                    let ids: Vec<String> = stmt
                        .query_map(params![attribute, &pat, coluuid], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect();
                    ids
                }
                Operator::Endswith => {
                    let pat = format!("%{}", subject);
                    let sql = if negation {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE NOT LIKE ?2 AND COLUUID = ?3;"
                    } else {
                        "SELECT OBJUUID FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND VALUE LIKE ?2 AND COLUUID = ?3;"
                    };
                    let mut stmt = conn.prepare(sql)?;
                    let ids: Vec<String> = stmt
                        .query_map(params![attribute, &pat, coluuid], |row| row.get(0))?
                        .filter_map(|r| r.ok())
                        .collect();
                    ids
                }
                // GT, GTE, LT, LTE, Inside, Regex — fetch all and filter in Rust
                _ => {
                    let mut stmt = conn.prepare(
                        "SELECT OBJUUID, VALUE FROM TBL_INDEX \
                         WHERE ATTRIBUTE = ?1 AND COLUUID = ?2;",
                    )?;
                    let rows: Vec<(String, String)> = stmt
                        .query_map(params![attribute, coluuid], |row| {
                            Ok((row.get(0)?, row.get(1)?))
                        })?
                        .filter_map(|r| r.ok())
                        .collect();

                    rows.into_iter()
                        .filter(|(_, value)| {
                            let append = match &operator {
                                Operator::Gt => compare_coerced(value, &subject)
                                    == Some(std::cmp::Ordering::Greater),
                                Operator::Gte => matches!(
                                    compare_coerced(value, &subject),
                                    Some(std::cmp::Ordering::Greater)
                                        | Some(std::cmp::Ordering::Equal)
                                ),
                                Operator::Lt => compare_coerced(value, &subject)
                                    == Some(std::cmp::Ordering::Less),
                                Operator::Lte => matches!(
                                    compare_coerced(value, &subject),
                                    Some(std::cmp::Ordering::Less)
                                        | Some(std::cmp::Ordering::Equal)
                                ),
                                Operator::Inside => subject.contains(value.as_str()),
                                Operator::Regex => Regex::new(&subject)
                                    .is_ok_and(|re| re.is_match(value)),
                                _ => false,
                            };
                            if negation { !append } else { append }
                        })
                        .map(|(id, _)| id)
                        .collect()
                }
            };

            objuuid_lists.push(ids);
        }

        if objuuid_lists.is_empty() {
            return Ok(vec![]);
        }

        // Intersect all result sets
        let mut result: std::collections::HashSet<String> =
            objuuid_lists[0].iter().cloned().collect();
        for list in &objuuid_lists[1..] {
            let set: std::collections::HashSet<String> = list.iter().cloned().collect();
            result = result.intersection(&set).cloned().collect();
        }
        let ids: Vec<String> = result.into_iter().collect();
        Ok(match limit {
            Some(n) => ids.into_iter().take(n).collect(),
            None => ids,
        })
    }

    // ── Attributes ────────────────────────────────────────────────────────────

    pub fn list_attributes(&self, coluuid: &str) -> Result<Vec<(String, String)>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT ATTRIBUTE, PATH FROM TBL_ATTRIBUTES WHERE COLUUID = ?1;")?;
        let result = stmt
            .query_map(params![coluuid], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn create_attribute(&self, coluuid: &str, attribute: &str, path: &str) -> Result<()> {
        if RESERVED_ATTRIBUTES.contains(&attribute) {
            return Err(anyhow!(
                "\"{}\" is a reserved attribute name and cannot be used as a collection attribute",
                attribute
            ));
        }
        let conn = self.connection.lock().unwrap();
        conn.execute(
            "INSERT INTO TBL_ATTRIBUTES (COLUUID, ATTRIBUTE, PATH) VALUES (?1, ?2, ?3);",
            params![coluuid, attribute, path],
        )?;

        // Index all existing objects for this new attribute
        let rows: Vec<(String, String)> = {
            let mut stmt = conn
                .prepare("SELECT OBJUUID, VALUE FROM TBL_OBJECTS WHERE COLUUID = ?1;")?;
            let x: Vec<(String, String)> = stmt
                .query_map(params![coluuid], |row| Ok((row.get(0)?, row.get(1)?)))?
                .filter_map(|r| r.ok())
                .collect();
            x
        };

        for (objuuid, value_text) in rows {
            if let Ok(value) = serde_json::from_str::<Value>(&value_text) {
                if let Some(v) = read_key_at_path(path, &value) {
                    let val_str = match &v {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    };
                    let _ = conn.execute(
                        "INSERT OR REPLACE INTO TBL_INDEX \
                         (OBJUUID, COLUUID, ATTRIBUTE, VALUE) VALUES (?1, ?2, ?3, ?4);",
                        params![&objuuid, coluuid, attribute, &val_str],
                    );
                }
            }
        }
        Ok(())
    }

    pub fn delete_attribute(&self, coluuid: &str, attribute: &str) -> Result<()> {
        if RESERVED_ATTRIBUTES.contains(&attribute) {
            return Err(anyhow!(
                "\"{}\" is a reserved attribute name and cannot be deleted",
                attribute
            ));
        }
        let conn = self.connection.lock().unwrap();
        conn.execute(
            "DELETE FROM TBL_ATTRIBUTES WHERE COLUUID = ?1 AND ATTRIBUTE = ?2;",
            params![coluuid, attribute],
        )?;
        conn.execute(
            "DELETE FROM TBL_INDEX WHERE ATTRIBUTE = ?1 AND COLUUID = ?2;",
            params![attribute, coluuid],
        )?;
        Ok(())
    }

    // ── Collections ───────────────────────────────────────────────────────────

    pub fn create_collection(&self, name: &str) -> Result<String> {
        let coluuid = Self::get_uuid();
        let conn = self.connection.lock().unwrap();
        conn.execute(
            "INSERT INTO TBL_COLLECTIONS (COLUUID, NAME) VALUES (?1, ?2);",
            params![&coluuid, name],
        )?;
        Ok(coluuid)
    }

    pub fn delete_collection(&self, coluuid: &str) -> Result<()> {
        let conn = self.connection.lock().unwrap();
        conn.execute(
            "DELETE FROM TBL_COLLECTIONS WHERE COLUUID = ?1;",
            params![coluuid],
        )?;
        Ok(())
    }

    pub fn list_collections(&self) -> Result<HashMap<String, String>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare("SELECT NAME, COLUUID FROM TBL_COLLECTIONS;")?;
        let result = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }

    pub fn list_collection_objects(&self, coluuid: &str) -> Result<Vec<String>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT OBJUUID FROM TBL_OBJECTS WHERE COLUUID = ?1;")?;
        let result = stmt
            .query_map(params![coluuid], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(result)
    }
}

// ── Operator parsing ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Operator {
    Eq,
    Gt,
    Gte,
    Lt,
    Lte,
    Inside,
    Contains,
    Startswith,
    Endswith,
    Regex,
}

/// Parse an expression string such as `"$gt:2"`, `"$!eq:red"`, or `"apple"`.
/// Returns `(operator, negated, subject)`.
pub(crate) fn parse_expression(expression: &str) -> (Operator, bool, String) {
    let (negation, op_start) = if expression.starts_with("$!") {
        (true, 2usize)
    } else if expression.starts_with('$') {
        (false, 1usize)
    } else {
        return (Operator::Eq, false, expression.to_string());
    };

    let colon_pos = match expression[op_start..].find(':') {
        Some(p) => op_start + p,
        None => return (Operator::Eq, negation, expression[op_start..].to_string()),
    };

    let op_str = &expression[op_start..colon_pos];
    let subject = expression[colon_pos + 1..].to_string();

    let operator = match op_str.to_uppercase().as_str() {
        "EQ" => Operator::Eq,
        "GT" => Operator::Gt,
        "GTE" => Operator::Gte,
        "LT" => Operator::Lt,
        "LTE" => Operator::Lte,
        "INSIDE" => Operator::Inside,
        "CONTAINS" => Operator::Contains,
        "STARTSWITH" => Operator::Startswith,
        "ENDSWITH" => Operator::Endswith,
        "REGEX" => Operator::Regex,
        _ => Operator::Eq,
    };
    (operator, negation, subject)
}

// ── Path traversal ────────────────────────────────────────────────────────────

/// Navigate a `serde_json::Value` along a path string such as `"/key1/0/key2"`.
/// The first character is the delimiter.
pub fn read_key_at_path(path: &str, value: &Value) -> Option<Value> {
    if path.is_empty() {
        return None;
    }
    let delim = &path[..1];
    let tokens: Vec<&str> = path[1..].split(delim).collect();
    let mut current = value.clone();
    for token in tokens {
        current = if let Ok(idx) = token.parse::<usize>() {
            current.get(idx)?.clone()
        } else {
            current.get(token)?.clone()
        };
    }
    Some(current)
}

// ── Coercion helpers ──────────────────────────────────────────────────────────

#[derive(Debug)]
enum CoercedValue {
    Int(i64),
    Float(f64),
    Str(String),
}

impl PartialEq for CoercedValue {
    fn eq(&self, other: &Self) -> bool {
        self.partial_cmp(other) == Some(std::cmp::Ordering::Equal)
    }
}

impl PartialOrd for CoercedValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (CoercedValue::Int(a), CoercedValue::Int(b)) => a.partial_cmp(b),
            (CoercedValue::Float(a), CoercedValue::Float(b)) => a.partial_cmp(b),
            (CoercedValue::Int(a), CoercedValue::Float(b)) => (*a as f64).partial_cmp(b),
            (CoercedValue::Float(a), CoercedValue::Int(b)) => a.partial_cmp(&(*b as f64)),
            (CoercedValue::Str(a), CoercedValue::Str(b)) => a.partial_cmp(b),
            _ => None,
        }
    }
}

fn coerce(s: &str) -> CoercedValue {
    if let Ok(i) = s.parse::<i64>() {
        CoercedValue::Int(i)
    } else if let Ok(f) = s.parse::<f64>() {
        CoercedValue::Float(f)
    } else {
        CoercedValue::Str(s.to_string())
    }
}

fn compare_coerced(a: &str, b: &str) -> Option<std::cmp::Ordering> {
    coerce(a).partial_cmp(&coerce(b))
}
