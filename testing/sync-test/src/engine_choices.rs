/* Any copyright is dedicated to the Public Domain.
http://creativecommons.org/publicdomain/zero/1.0/ */

//! Tests for clients with different engine choice models sharing an account,
//! as will happen while per-device engine choices roll out. Each `TestClient`
//! can be made an "old" (per-account) or "new" (per-device) client by setting
//! its `engine_choices_model`, and these tests check what each leaves on the
//! server for the other.
//!
//! Note that these can't simulate Desktop - only the states Desktop would see.

use crate::auth::TestClient;
use crate::testing::TestGroup;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use sync15::EngineChoicesModel;
use sync_manager::SyncResult;

const ENGINES: &[&str] = &["tabs", "passwords"];

// These tests need to know exactly what's on the server, so start from nothing.
// (The server is also wiped after each test, but that doesn't help if a
// previous run didn't finish.)
fn start_clean(c: &mut TestClient) {
    c.fully_wipe_server().expect("should wipe");
}

fn sync(c: &mut TestClient, engines: &[&str]) -> SyncResult {
    let engines: Vec<String> = engines.iter().map(ToString::to_string).collect();
    c.sync(&engines, HashMap::new()).expect("should sync")
}

// Syncs a per-account client which the user has just changed engine choices on.
fn sync_with_changes(c: &mut TestClient, changes: &[(&str, bool)]) -> SyncResult {
    c.engine_choices_model = EngineChoicesModel::Account {
        enabled_changes: changes.iter().map(|(n, v)| (n.to_string(), *v)).collect(),
    };
    let result = sync(c, ENGINES);
    // The app only sends changes once.
    c.engine_choices_model = EngineChoicesModel::default();
    result
}

fn meta_global(c: &TestClient) -> Value {
    c.server_meta_global()
        .expect("should fetch")
        .expect("should have meta/global")
}

fn sync_ids(c: &TestClient) -> Value {
    c.server_sync_ids()
        .expect("should fetch")
        .expect("should have meta/syncids")
}

fn sync_id<'a>(record: &'a Value, engine: &str) -> Option<&'a str> {
    record["engines"][engine]["syncID"].as_str()
}

fn declined(meta_global: &Value) -> HashSet<String> {
    meta_global["declined"]
        .as_array()
        .expect("should have declined")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect()
}

fn synced(result: &SyncResult, engine: &str) -> bool {
    result.successful.iter().any(|e| e == engine)
}

fn result_declined(result: &SyncResult) -> HashSet<String> {
    result
        .declined
        .as_ref()
        .expect("per-account clients report declined engines")
        .iter()
        .cloned()
        .collect()
}

fn set(names: &[&str]) -> HashSet<String> {
    names.iter().map(ToString::to_string).collect()
}

// A per-device client creates the account's meta/global, then a per-account
// client joins.
fn test_per_device_first_then_per_account(c0: &mut TestClient, c1: &mut TestClient) {
    start_clean(c0);
    c0.engine_choices_model = EngineChoicesModel::Device;
    let result = sync(c0, ENGINES);
    assert!(result.declined.is_none());
    assert!(synced(&result, "tabs") && synced(&result, "passwords"));

    // No `engines` at all - Desktop treats an empty one as "everything disabled".
    let global = meta_global(c0);
    assert!(global.get("engines").is_none(), "{global}");
    assert!(declined(&global).is_empty());
    let ids = sync_ids(c0);
    let tabs_id = sync_id(&ids, "tabs")
        .expect("tabs has a sync ID")
        .to_string();
    assert!(sync_id(&ids, "passwords").is_some());

    // The per-account client fills in meta/global, using the per-device client's sync IDs.
    let result = sync(c1, ENGINES);
    assert_eq!(result_declined(&result), set(&[]));
    let global = meta_global(c1);
    assert_eq!(sync_id(&global, "tabs"), Some(tabs_id.as_str()));
    assert_eq!(sync_id(&global, "passwords"), sync_id(&ids, "passwords"));

    // Which the per-device client is already using, so nothing changes.
    sync(c0, ENGINES);
    assert_eq!(meta_global(c0), global);
    assert_eq!(sync_ids(c0), ids);
}

// A per-device client syncs an engine the account declined, without changing
// the account's choices.
fn test_per_device_ignores_declined(c0: &mut TestClient, c1: &mut TestClient) {
    start_clean(c0);
    let result = sync_with_changes(c1, &[("tabs", false)]);
    assert!(!synced(&result, "tabs"));
    let global = meta_global(c1);
    assert!(declined(&global).contains("tabs"));

    c0.engine_choices_model = EngineChoicesModel::Device;
    let result = sync(c0, ENGINES);
    assert!(synced(&result, "tabs"));
    assert!(result.declined.is_none());
    // meta/global is never changed by a per-device client...
    assert_eq!(meta_global(c0), global);
    // ...which instead keeps sync IDs in meta/syncids.
    let ids = sync_ids(c0);
    assert!(sync_id(&ids, "tabs").is_some());
    assert_eq!(sync_id(&ids, "passwords"), sync_id(&global, "passwords"));

    // And the per-account client still has tabs declined.
    let result = sync(c1, ENGINES);
    assert!(result_declined(&result).contains("tabs"));
    assert!(!synced(&result, "tabs"));
}

// A per-device client recreates meta/global (eg, after a node reassignment) -
// its empty declined list must not enable everything on per-account clients.
fn test_per_account_keeps_declined_after_per_device_fresh_start(
    c0: &mut TestClient,
    c1: &mut TestClient,
) {
    start_clean(c0);
    sync_with_changes(c1, &[("tabs", false)]);

    // As if the per-device client found the server had been wiped.
    c0.fully_wipe_server().expect("should wipe");
    c0.engine_choices_model = EngineChoicesModel::Device;
    sync(c0, &["passwords"]);
    let global = meta_global(c0);
    assert!(global.get("engines").is_none(), "{global}");
    assert!(declined(&global).is_empty());

    let result = sync(c1, ENGINES);
    assert!(result_declined(&result).contains("tabs"));
    assert!(!synced(&result, "tabs"));
    let global = meta_global(c1);
    assert!(declined(&global).contains("tabs"));
    assert!(sync_id(&global, "tabs").is_none());
    assert_eq!(
        sync_id(&global, "passwords"),
        sync_id(&sync_ids(c0), "passwords")
    );
}

// A client flips from per-device to per-account choices and back, as it might
// while testing the pref.
fn test_flip_per_device_to_per_account(c0: &mut TestClient, c1: &mut TestClient) {
    start_clean(c0);
    sync_with_changes(c1, &[("tabs", false)]);

    c0.engine_choices_model = EngineChoicesModel::Device;
    assert!(synced(&sync(c0, ENGINES), "tabs"));
    let tabs_id = sync_id(&sync_ids(c0), "tabs").unwrap().to_string();

    // Per-account, it rejoins the account's choices.
    c0.engine_choices_model = EngineChoicesModel::default();
    let result = sync(c0, ENGINES);
    assert!(result_declined(&result).contains("tabs"));
    assert!(!synced(&result, "tabs"));
    assert!(sync_id(&meta_global(c0), "tabs").is_none());

    // And per-device again, it carries on with the same sync ID.
    c0.engine_choices_model = EngineChoicesModel::Device;
    assert!(synced(&sync(c0, ENGINES), "tabs"));
    assert_eq!(sync_id(&sync_ids(c0), "tabs"), Some(tabs_id.as_str()));
}

pub fn get_test_group() -> TestGroup {
    TestGroup::new(
        "engine-choices",
        vec![
            (
                "test_per_device_first_then_per_account",
                test_per_device_first_then_per_account,
            ),
            (
                "test_per_device_ignores_declined",
                test_per_device_ignores_declined,
            ),
            (
                "test_per_account_keeps_declined_after_per_device_fresh_start",
                test_per_account_keeps_declined_after_per_device_fresh_start,
            ),
            (
                "test_flip_per_device_to_per_account",
                test_flip_per_device_to_per_account,
            ),
        ],
    )
}
