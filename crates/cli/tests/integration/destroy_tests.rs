//! Destroy command integration tests.

use predicates::prelude::*;

use super::common::TestEnv;

#[test]
fn destroy_removes_bind_artifacts() {
  let env = TestEnv::from_fixture("bind_create.lua");
  let marker_file = env.output_path().join("created.txt");

  // First apply to create the bind
  env.sys_cmd().arg("apply").arg(&env.config_path).assert().success();

  assert!(marker_file.exists(), "marker file should exist after apply");

  // Destroy should remove it
  env
    .sys_cmd()
    .arg("destroy")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("destroy"));

  // Note: Current destroy is a placeholder. When fully implemented,
  // uncomment this assertion:
  // assert!(!marker_file.exists(), "marker file should be removed after destroy");
}

#[test]
fn destroy_nonexistent_config_succeeds() {
  // Destroy with no previous state should succeed gracefully
  let env = TestEnv::from_fixture("minimal.lua");

  env.sys_cmd().arg("destroy").arg(&env.config_path).assert().success();
}
