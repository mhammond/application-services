# Shared Settings

This is an EXPERIMENTAL proof-of-concept for a "settings" API that syncs.
It's different from desktop's "preferences" engine in that instead of syncing implementation
details about what exact preference names are used, it is intended to sync concepts which apply
to all platforms - hence "shared": the settings are shared between desktop, Android and iOS,
rather than being private to one app.

For example, you could imagine the (now dated) "world cup widget" might want
to sync the teams you follow.

## Syncing

Settings sync to their own Sync collection, `shared-settings`. We reuse
webext-storage's storage format and sync implementation (including its 3-way
merge), just pointed at that collection instead of `extension-storage` - see
`WebExtStorageStore::create_sync_engine()`.

Only the mobile "sync manager" is supported: call `registerWithSyncManager()`
on the store, then include `shared-settings` in the engines you ask the sync
manager to sync. There's deliberately no `BridgedEngine`, so Desktop can't
drive this yet.
