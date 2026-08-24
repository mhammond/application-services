/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use anyhow::Result;
use rusqlite::Transaction;
use std::sync::{Arc, Weak};
use sync15::bso::{IncomingBso, OutgoingBso};
use sync15::engine::{CollSyncIds, CollectionRequest, EngineSyncAssociation, SyncEngine};
use sync15::{telemetry, CollectionName, ServerTimestamp};
use sync_guid::Guid as SyncGuid;

use crate::db::{delete_meta, get_meta, put_meta, ThreadSafeStorageDb};
use crate::schema;
use crate::sync::incoming::{apply_actions, get_incoming, plan_incoming, stage_incoming};
use crate::sync::outgoing::{get_outgoing, record_uploaded, stage_outgoing};

pub(crate) const LAST_SYNC_META_KEY: &str = "last_sync_time";
pub(crate) const SYNC_ID_META_KEY: &str = "sync_id";

/// The sync engine for this storage format. The collection it syncs to is
/// supplied by the caller: Desktop's `storage.sync` uses `extension-storage`,
/// while components which wrap this store (eg, `shared-settings`) reuse the
/// same machinery against a collection of their own.
pub struct WebExtSyncEngine {
    db: Weak<ThreadSafeStorageDb>,
    collection_name: CollectionName,
}

impl WebExtSyncEngine {
    pub fn new(db: &Arc<ThreadSafeStorageDb>, collection_name: CollectionName) -> Self {
        WebExtSyncEngine {
            db: Arc::downgrade(db),
            collection_name,
        }
    }

    fn do_reset(&self, tx: &Transaction<'_>) -> Result<()> {
        tx.execute_batch(
            "DELETE FROM storage_sync_mirror;
             UPDATE storage_sync_data SET sync_change_counter = 1;",
        )?;
        delete_meta(tx, LAST_SYNC_META_KEY)?;
        Ok(())
    }

    fn thread_safe_storage_db(&self) -> Result<Arc<ThreadSafeStorageDb>> {
        self.db
            .upgrade()
            .ok_or_else(|| crate::error::Error::DatabaseConnectionClosed.into())
    }
}

impl SyncEngine for WebExtSyncEngine {
    fn collection_name(&self) -> CollectionName {
        self.collection_name.clone()
    }

    // Read-only view of the engine-owned last-sync time, for the Desktop bridge.
    // It's written only internally, in `apply`/`set_uploaded`.
    fn last_sync(&self) -> Result<Option<ServerTimestamp>> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        Ok(get_meta::<i64>(conn, LAST_SYNC_META_KEY)?.map(ServerTimestamp))
    }

    fn reset_last_sync(&self) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        delete_meta(&tx, LAST_SYNC_META_KEY)?;
        tx.commit()?;
        Ok(())
    }

    fn get_sync_assoc(&self) -> Result<EngineSyncAssociation> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        // Bridged engines never maintain the "global" guid - that's all managed
        // by the consumer (Desktop); they only care about the per-collection one.
        Ok(match get_meta::<String>(conn, SYNC_ID_META_KEY)? {
            Some(coll) => EngineSyncAssociation::Connected(CollSyncIds {
                global: SyncGuid::empty(),
                coll: coll.into(),
            }),
            None => EngineSyncAssociation::Disconnected,
        })
    }

    fn sync_started(&self) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        schema::create_empty_sync_temp_tables(conn)?;
        Ok(())
    }

    fn stage_incoming(
        &self,
        incoming_bsos: Vec<IncomingBso>,
        _telem: &mut telemetry::Engine,
    ) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let signal = db.begin_interrupt_scope()?;
        let conn = db.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        let incoming_content: Vec<_> = incoming_bsos
            .into_iter()
            .map(IncomingBso::into_content::<super::WebextRecord>)
            .collect();
        stage_incoming(&tx, &incoming_content, &signal)?;
        tx.commit()?;
        Ok(())
    }

    fn apply(
        &self,
        timestamp: ServerTimestamp,
        _telem: &mut telemetry::Engine,
    ) -> Result<Vec<OutgoingBso>> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let signal = db.begin_interrupt_scope()?;
        let conn = db.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        let incoming = get_incoming(&tx)?;
        let actions = incoming
            .into_iter()
            .map(|(item, state)| (item, plan_incoming(state)))
            .collect();
        apply_actions(&tx, actions, &signal)?;
        stage_outgoing(&tx)?;
        // The engine owns its last-sync time: record the collection timestamp we
        // just synced to, so it advances without any external `set_last_sync`.
        // (Timestamp is zero only in an upload-only path, which must not move it.)
        if timestamp != ServerTimestamp(0) {
            put_meta(&tx, LAST_SYNC_META_KEY, &timestamp.as_millis())?;
        }
        tx.commit()?;

        Ok(get_outgoing(conn, &signal)?)
    }

    fn set_uploaded(&self, new_timestamp: ServerTimestamp, ids: Vec<SyncGuid>) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        let signal = db.begin_interrupt_scope()?;
        let tx = conn.unchecked_transaction()?;
        record_uploaded(&tx, &ids, &signal)?;
        // Advance the engine-owned last-sync time to the post-upload timestamp.
        if new_timestamp != ServerTimestamp(0) {
            put_meta(&tx, LAST_SYNC_META_KEY, &new_timestamp.as_millis())?;
        }
        tx.commit()?;

        Ok(())
    }

    fn sync_finished(&self) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        schema::create_empty_sync_temp_tables(conn)?;
        Ok(())
    }

    fn get_collection_request(
        &self,
        server_timestamp: ServerTimestamp,
    ) -> Result<Option<CollectionRequest>> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        let since = ServerTimestamp(get_meta::<i64>(conn, LAST_SYNC_META_KEY)?.unwrap_or(0));
        Ok(if since == server_timestamp {
            None
        } else {
            Some(
                CollectionRequest::new(self.collection_name())
                    .full()
                    .newer_than(since),
            )
        })
    }

    fn reset(&self, assoc: &EngineSyncAssociation) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        self.do_reset(&tx)?;
        // A `Disconnected` reset clears the sync ID; a `Connected` one adopts the
        // (per-collection) ID. `do_reset` already cleared the last sync time.
        match assoc {
            EngineSyncAssociation::Disconnected => {
                delete_meta(&tx, SYNC_ID_META_KEY)?;
            }
            EngineSyncAssociation::Connected(ids) => {
                put_meta(&tx, SYNC_ID_META_KEY, &ids.coll.to_string())?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn wipe(&self) -> Result<()> {
        let shared_db = self.thread_safe_storage_db()?;
        let db = shared_db.lock();
        let conn = db.get_connection()?;
        let tx = conn.unchecked_transaction()?;
        // We assume the meta table is only used by sync.
        tx.execute_batch(
            "DELETE FROM storage_sync_data; DELETE FROM storage_sync_mirror; DELETE FROM meta;",
        )?;
        tx.commit()?;
        Ok(())
    }
}

impl From<anyhow::Error> for crate::error::Error {
    fn from(value: anyhow::Error) -> Self {
        crate::error::Error::SyncError(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test::new_mem_thread_safe_storage_db;

    #[test]
    fn test_collection_name_is_supplied_by_the_caller() {
        let db = new_mem_thread_safe_storage_db();
        let engine = WebExtSyncEngine::new(&db, "shared-settings".into());
        assert_eq!(engine.collection_name(), "shared-settings");
    }
}
