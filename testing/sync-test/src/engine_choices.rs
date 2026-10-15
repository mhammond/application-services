/* Any copyright is dedicated to the Public Domain.
http://creativecommons.org/publicdomain/zero/1.0/ */

//! Tests for clients with different engine choice models sharing an account,
//! as will happen while per-device engine choices roll out. These check what
//! "old" (per-account) and "new" (per-device) clients leave on the server for
//! each other.
//!
//! Note that these can't simulate Desktop - only the states Desktop would see.

use crate::auth::TestClient;
use crate::testing::TestGroup;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use sync15::EngineChoicesModel::{self, Device};
use sync_manager::ServiceStatus;

// Syncs tabs and passwords with the given engine choices model, returning the
// engines which actually synced. `EngineChoicesModel::default()` is a per-account
// client with no changes to make.
fn sync(c: &mut TestClient, model: EngineChoicesModel) -> HashSet<String> {
    c.engine_choices_model = model;
    let engines = vec!["tabs".to_string(), "passwords".to_string()];
    let result = c.sync(&engines, HashMap::new()).expect("should sync");
    assert!(matches!(result.status, ServiceStatus::Ok));
    assert_eq!(result.failures.len(), 0);
    result.successful.into_iter().collect()
}

fn meta_global(c: &TestClient) -> Value {
    c.server_meta_global()
        .expect("should fetch")
        .expect("should have meta/global")
}

fn meta_syncids(c: &TestClient) -> Value {
    c.server_sync_ids()
        .expect("should fetch")
        .expect("should have meta/syncids")
}

fn sync_id(record: &Value, engine: &str) -> Option<String> {
    record["engines"][engine]["syncID"]
        .as_str()
        .map(ToString::to_string)
}

// A per-device client's fresh start puts the engines it syncs in meta/global,
// and a per-account client then syncs them too.
fn test_per_device_fresh_start(c0: &mut TestClient, c1: &mut TestClient) {
    sync(c0, Device);
    let global = meta_global(c0);
    assert!(sync_id(&global, "tabs").is_some());
    assert_eq!(sync_id(&meta_syncids(c0), "tabs"), sync_id(&global, "tabs"));

    assert!(sync(c1, EngineChoicesModel::default()).contains("tabs"));
    assert_eq!(meta_global(c1), global);
}

// The same happens when the account had disabled the engine, if the per-device
// client has never seen a meta/global.
fn test_per_device_fresh_start_enables_for_account(c0: &mut TestClient, c1: &mut TestClient) {
    // c1 is a per-account client on which the user disabled tabs.
    let disable_tabs = EngineChoicesModel::Account {
        enabled_changes: HashMap::from([("tabs".to_string(), false)]),
    };
    sync(c1, disable_tabs);

    c0.fully_wipe_server().expect("should wipe");
    sync(c0, Device);

    assert!(sync(c1, EngineChoicesModel::default()).contains("tabs"));
    assert_eq!(
        sync_id(&meta_global(c1), "tabs"),
        sync_id(&meta_syncids(c0), "tabs")
    );
}

// But a per-device client which has seen the account's choices republishes
// them - eg, after finding the server wiped after a node reassignment.
fn test_per_device_fresh_start_keeps_account_choices(c0: &mut TestClient, c1: &mut TestClient) {
    // c1 is a per-account client on which the user disabled tabs.
    let disable_tabs = EngineChoicesModel::Account {
        enabled_changes: HashMap::from([("tabs".to_string(), false)]),
    };
    sync(c1, disable_tabs);
    sync(c0, Device);

    c0.fully_wipe_server().expect("should wipe");
    sync(c0, Device);

    let global = meta_global(c0);
    assert!(sync_id(&global, "tabs").is_none());
    assert!(global["declined"]
        .as_array()
        .expect("should have declined")
        .contains(&Value::from("tabs")));
    // c0 still syncs tabs, using meta/syncids.
    assert!(sync_id(&meta_syncids(c0), "tabs").is_some());

    assert!(!sync(c1, EngineChoicesModel::default()).contains("tabs"));
}

// Otherwise a per-device client never changes meta/global - it syncs an engine
// the account disabled using a sync ID from meta/syncids.
fn test_per_device_leaves_account_choices(c0: &mut TestClient, c1: &mut TestClient) {
    // c1 is a per-account client on which the user disabled tabs.
    let disable_tabs = EngineChoicesModel::Account {
        enabled_changes: HashMap::from([("tabs".to_string(), false)]),
    };
    sync(c1, disable_tabs);
    let global = meta_global(c1);

    sync(c0, Device);
    assert_eq!(meta_global(c0), global);
    assert!(sync_id(&meta_syncids(c0), "tabs").is_some());

    assert!(!sync(c1, EngineChoicesModel::default()).contains("tabs"));
}

pub fn get_test_group() -> TestGroup {
    TestGroup::new(
        "engine-choices",
        vec![
            ("test_per_device_fresh_start", test_per_device_fresh_start),
            (
                "test_per_device_fresh_start_enables_for_account",
                test_per_device_fresh_start_enables_for_account,
            ),
            (
                "test_per_device_fresh_start_keeps_account_choices",
                test_per_device_fresh_start_keeps_account_choices,
            ),
            (
                "test_per_device_leaves_account_choices",
                test_per_device_leaves_account_choices,
            ),
        ],
    )
}
