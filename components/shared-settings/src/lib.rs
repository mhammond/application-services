/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use serde_json::Value as JsonValue;
use std::sync::Arc;
use webext_storage::WebExtStorageStore;

mod sync;

pub use sync::get_registered_sync_engine;

uniffi::custom_type!(JsonValue, String, {
    remote,
    try_lift: |val| Ok(serde_json::from_str(val.as_str())?),
    lower: |obj| obj.to_string(),
});

type Result<T> = std::result::Result<T, SharedSettingsApiError>;

// This is an experimental component, so it uses a single "flat" error rather than the
// internal/public error split described in `components/example/src/error.rs`. Adopt that
// split (and the error reporting it brings) if this component graduates.
#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum SharedSettingsApiError {
    #[error("webext-storage error: {0:?}")]
    StorageError(webext_storage::error::Error),
}

impl From<webext_storage::error::Error> for SharedSettingsApiError {
    fn from(e: webext_storage::error::Error) -> Self {
        SharedSettingsApiError::StorageError(e)
    }
}

#[derive(uniffi::Object)]
pub struct SharedSettingsStore {
    pub(crate) webext_store: Arc<WebExtStorageStore>,
}

#[uniffi::export]
impl SharedSettingsStore {
    /// Creates a store backed by a database at `db_path`. The path can be a
    /// file path or `file:` URI.
    #[uniffi::constructor]
    pub fn new(db_path: &str) -> Result<Self> {
        let webext_store = Arc::new(WebExtStorageStore::new(db_path)?);
        Ok(Self { webext_store })
    }

    // Creates a store backed by an in-memory database.
    // #[cfg(test)]
    // pub fn new_memory(db_path: &str) -> Result<Self> {
    //     let db = StorageDb::new_memory(db_path)?;
    //     Ok(Self {
    //         db: Arc::new(ThreadSafeStorageDb::new(db)),
    //     })
    // }

    // /// Returns an interrupt handle for this store.
    // pub fn interrupt_handle(&self) -> Arc<SqlInterruptHandle> {
    //     self.db.interrupt_handle()
    // }

    /// Sets one or more JSON key-value pairs for an extension ID. Returns a
    /// list of changes, with existing and new values for each key in `val`.
    pub fn set(&self, namespace: &str, val: JsonValue) -> Result<()> {
        self.webext_store.set(namespace, val)?;
        Ok(())
    }

    /// Returns the values for one or more keys `keys` can be:
    ///
    /// - `null`, in which case all key-value pairs for the extension are
    ///   returned, or an empty object if the extension doesn't have any
    ///   stored data.
    /// - A single string key, in which case an object with only that key
    ///   and its value is returned, or an empty object if the key doesn't
    //    exist.
    /// - An array of string keys, in which case an object with only those
    ///   keys and their values is returned. Any keys that don't exist will be
    ///   omitted.
    /// - An object where the property names are keys, and each value is the
    ///   default value to return if the key doesn't exist.
    ///
    /// This method always returns an object (that is, a
    /// `serde_json::Value::Object`).
    pub fn get(&self, namespace: &str, keys: JsonValue) -> Result<JsonValue> {
        Ok(self.webext_store.get(namespace, keys)?)
    }

    /*
        /// Returns the keys for a given extension ID.
        pub fn get_keys(&self, ext_id: &str) -> Result<JsonValue> {
            let db = &self.db.lock();
            let conn = db.get_connection()?;
            api::get_keys(conn, ext_id)
        }

        /// Deletes the values for one or more keys. As with `get`, `keys` can be
        /// either a single string key, or an array of string keys. Returns a list
        /// of changes, where each change contains the old value for each deleted
        /// key.
        pub fn remove(&self, ext_id: &str, keys: JsonValue) -> Result<StorageChanges> {
            let db = &self.db.lock();
            let conn = db.get_connection()?;
            let tx = conn.unchecked_transaction()?;
            let result = api::remove(&tx, ext_id, keys)?;
            tx.commit()?;
            Ok(result)
        }

        /// Deletes all key-value pairs for the extension. As with `remove`, returns
        /// a list of changes, where each change contains the old value for each
        /// deleted key.
        pub fn clear(&self, ext_id: &str) -> Result<StorageChanges> {
            let db = &self.db.lock();
            let conn = db.get_connection()?;
            let tx = conn.unchecked_transaction()?;
            let result = api::clear(&tx, ext_id)?;
            tx.commit()?;
            Ok(result)
        }

        /// Returns the bytes in use for the specified items (which can be null,
        /// a string, or an array)
        pub fn get_bytes_in_use(&self, ext_id: &str, keys: JsonValue) -> Result<u64> {
            let db = &self.db.lock();
            let conn = db.get_connection()?;
            Ok(api::get_bytes_in_use(conn, ext_id, keys)? as u64)
        }
    */
    /// Closes the store and its database connection. See the docs for
    /// `StorageDb::close` for more details on when this can fail.
    ///
    /// Not named `close` because UniFFI's Kotlin bindings already generate a
    /// `close()` for `AutoCloseable`, which frees the object rather than the database.
    pub fn shutdown(&self) -> Result<()> {
        Ok(self.webext_store.close()?)
    }

    /*
        /// Gets the changes which the current sync applied. Should be used
        /// immediately after the bridged engine is told to apply incoming changes,
        /// and can be used to notify observers of the StorageArea of the changes
        /// that were applied.
        /// The result is a Vec of already JSON stringified changes.
        pub fn get_synced_changes(&self) -> Result<Vec<sync::SyncedExtensionChange>> {
            let db = self.db.lock();
            sync::get_synced_changes(&db)
        }

        /// Migrates data from a database in the format of the "old" kinto
        /// implementation. Information about how the migration went is stored in
        /// the database, and can be read using `Self::take_migration_info`.
        ///
        /// Note that `filename` isn't normalized or canonicalized.
        pub fn migrate(&self, filename: impl AsRef<Path>) -> Result<()> {
            let db = &self.db.lock();
            let conn = db.get_connection()?;
            let tx = conn.unchecked_transaction()?;
            let result = migrate(&tx, filename.as_ref())?;
            tx.commit()?;
            // Failing to store this information should not cause migration failure.
            if let Err(e) = result.store(conn) {
                debug_assert!(false, "Migration error: {:?}", e);
                warn!("Failed to record migration telmetry: {}", e);
            }
            Ok(())
        }

        /// Read-and-delete (e.g. `take` in rust parlance, see Option::take)
        /// operation for any MigrationInfo stored in this database.
        pub fn take_migration_info(&self) -> Result<Option<MigrationInfo>> {
            let db = &self.db.lock();
            let conn = db.get_connection()?;
            let tx = conn.unchecked_transaction()?;
            let result = MigrationInfo::take(&tx)?;
            tx.commit()?;
            Ok(result)
        }
    */
}

uniffi::setup_scaffolding!();

// wrappers around webext-storage's test helpers.
#[cfg(test)]
pub mod test {
    use super::*;

    pub fn new_mem_store() -> SharedSettingsStore {
        error_support::init_for_tests();
        let db = webext_storage::new_mem_thread_safe_storage_db();
        let webext_store = WebExtStorageStore::new_from_db(db);
        SharedSettingsStore {
            webext_store: Arc::new(webext_store),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test::*;
    use serde_json::json;

    #[test]
    fn test_simple() {
        let store = new_mem_store();
        store.set("test", json!({"value": "foo"})).unwrap();
        assert_eq!(
            store.get("test", json!(null)).unwrap(),
            json!({"value": "foo"})
        )
    }
}
