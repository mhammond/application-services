/* Any copyright is dedicated to the Public Domain.
http://creativecommons.org/publicdomain/zero/1.0/ */

use crate::auth::TestClient;
use crate::testing::TestGroup;
use anyhow::Result;
use serde_json::{json, Value as JsonValue};
use std::collections::HashMap;

const NAMESPACE: &str = "sync-test";

fn sync_shared_settings(client: &mut TestClient) -> Result<()> {
    client.sync(&["shared-settings".to_string()], HashMap::new())?;
    Ok(())
}

fn get_all(client: &TestClient) -> JsonValue {
    client
        .shared_settings_store
        .get(NAMESPACE, json!(null))
        .expect("should read settings")
}

fn set(client: &TestClient, val: JsonValue) {
    client
        .shared_settings_store
        .set(NAMESPACE, val)
        .expect("should write settings");
}

// Actual tests.

fn test_shared_settings(c0: &mut TestClient, c1: &mut TestClient) {
    log::info!("Set a setting on c0 and sync it to c1");
    set(c0, json!({"a-setting": "a-value"}));

    sync_shared_settings(c0).expect("c0 sync to work");
    sync_shared_settings(c1).expect("c1 sync to work");

    assert_eq!(get_all(c1), json!({"a-setting": "a-value"}));

    log::info!("Change a different key on each client and check they merge");
    set(c0, json!({"only-on-c0": "c0"}));
    set(c1, json!({"only-on-c1": "c1"}));

    sync_shared_settings(c0).expect("c0 sync to work");
    // c1 pulls c0's change, merges it with its own, and uploads the result.
    sync_shared_settings(c1).expect("c1 sync to work");
    // which c0 then picks up.
    sync_shared_settings(c0).expect("c0 sync to work");

    let expected = json!({
        "a-setting": "a-value",
        "only-on-c0": "c0",
        "only-on-c1": "c1",
    });
    assert_eq!(get_all(c0), expected);
    assert_eq!(get_all(c1), expected);
}

pub fn get_test_group() -> TestGroup {
    TestGroup::new(
        "shared-settings",
        vec![("test_shared_settings", test_shared_settings)],
    )
}
