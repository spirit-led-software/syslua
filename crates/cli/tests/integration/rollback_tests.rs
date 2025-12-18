//! Rollback behavior integration tests.

use super::common::TestEnv;

#[test]
fn rollback_restores_destroyed_binds_on_failure() {
  let env = TestEnv::from_fixture("rollback_bind_failure.lua");
  let marker_file = env.output_path().join("original.txt");

  // Phase 1: Create the original bind
  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .env("TEST_PHASE", "initial")
    .assert()
    .success();

  assert!(marker_file.exists(), "original.txt should exist after initial apply");

  // Phase 2: Apply with failing bind (should trigger rollback)
  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .env("TEST_PHASE", "failure")
    .assert()
    .failure();

  // Original bind should be restored
  assert!(marker_file.exists(), "original.txt should be restored after rollback");
}

#[test]
fn build_failure_skips_dependent_binds() {
  let env = TestEnv::from_fixture("rollback_build_failure.lua");
  let marker_file = env.output_path().join("should-not-exist.txt");

  env.sys_cmd().arg("apply").arg(&env.config_path).assert().failure();

  assert!(!marker_file.exists(), "dependent bind should not have run");
}
