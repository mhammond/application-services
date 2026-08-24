/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

//! Sync support for shared settings.
//!
//! We don't need a `BridgedEngine` here - we are targeting Android first, so
//! the only consumer is the sync manager, which drives plain `SyncEngine`s.
//! The engine itself is webext-storage's, pointed at a collection of our own.

use crate::SharedSettingsStore;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};
use sync15::engine::{SyncEngine, SyncEngineId};

/// The Sync collection we sync to. Note this is deliberately *not*
/// webext-storage's `extension-storage`: we reuse the storage format and the
/// sync machinery, but the data is ours.
pub const COLLECTION_NAME: &str = "shared-settings";

// Our "sync manager" will use whatever is stashed here.
lazy_static::lazy_static! {
    // Mutex: just taken long enough to update the inner stuff
    static ref STORE_FOR_MANAGER: Mutex<Weak<SharedSettingsStore>> = Mutex::new(Weak::new());
}

/// Called by the sync manager to get a sync engine via the store previously
/// registered with the sync manager.
pub fn get_registered_sync_engine(engine_id: &SyncEngineId) -> Option<Box<dyn SyncEngine>> {
    let weak = STORE_FOR_MANAGER.lock();
    match weak.upgrade() {
        None => None,
        Some(store) => match engine_id {
            SyncEngineId::SharedSettings => Some(
                store
                    .webext_store
                    .create_sync_engine(COLLECTION_NAME.into()),
            ),
            // panicking here seems reasonable - it's a static error if this
            // it hit, not something that runtime conditions can influence.
            _ => unreachable!("can't provide unknown engine: {}", engine_id),
        },
    }
}

#[uniffi::export]
impl SharedSettingsStore {
    /// Registers this store with the sync manager, so that syncs which include
    /// the `shared-settings` engine can find it. The manager only keeps a weak
    /// reference, so this doesn't keep the store alive.
    pub fn register_with_sync_manager(self: Arc<Self>) {
        let mut state = STORE_FOR_MANAGER.lock();
        *state = Arc::downgrade(&self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::new_mem_store;

    // A single test, because `STORE_FOR_MANAGER` is global - two tests touching
    // it would race with each other.
    #[test]
    fn test_registration() {
        let store = Arc::new(new_mem_store());
        assert_eq!(Arc::strong_count(&store), 1);
        assert_eq!(Arc::weak_count(&store), 0);
        Arc::clone(&store).register_with_sync_manager();
        assert_eq!(Arc::strong_count(&store), 1);
        assert_eq!(Arc::weak_count(&store), 1);
        let registered = STORE_FOR_MANAGER.lock().upgrade().expect("should upgrade");
        assert!(Arc::ptr_eq(&store, &registered));
        drop(registered);
        // should be no new references
        assert_eq!(Arc::strong_count(&store), 1);
        assert_eq!(Arc::weak_count(&store), 1);

        // and the engine the sync manager gets must be for our collection.
        let engine = get_registered_sync_engine(&SyncEngineId::SharedSettings)
            .expect("should have an engine");
        assert_eq!(engine.collection_name(), COLLECTION_NAME);
        drop(engine);

        // dropping the registered object should drop the registration.
        drop(store);
        assert!(STORE_FOR_MANAGER.lock().upgrade().is_none());
    }
}
