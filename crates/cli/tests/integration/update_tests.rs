//! Update command integration tests.

use predicates::prelude::*;

use super::common::TestEnv;

#[test]
fn update_bind_with_version_change() {
  let env = TestEnv::from_fixture("bind_update.lua");
  let version_file = env.output_path().join("version.txt");

  // Initial apply with v1
  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .env("TEST_VERSION", "v1")
    .assert()
    .success();

  assert!(version_file.exists(), "version.txt should exist after initial apply");
  let content_v1 = std::fs::read_to_string(&version_file).unwrap();
  assert!(content_v1.contains("v1"), "should contain v1");

  // Apply with v2 - should trigger update
  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .env("TEST_VERSION", "v2")
    .assert()
    .success()
    .stdout(predicate::str::contains("Binds updated: 1"));

  let content_v2 = std::fs::read_to_string(&version_file).unwrap();
  assert!(content_v2.contains("v2"), "should contain v2 after update");
}

#[test]
fn update_command_with_no_inputs() {
  let env = TestEnv::from_fixture("minimal.lua");

  env
    .sys_cmd()
    .arg("update")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("up to date"));
}
