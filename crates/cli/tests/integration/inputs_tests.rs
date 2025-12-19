//! Input resolution integration tests.
//!
//! These tests are marked `#[ignore]` because they require network access
//! and may be slow. Run with: `cargo test -- --ignored`

use predicates::prelude::*;

use super::common::TestEnv;

#[test]
#[ignore] // Requires network access
fn git_input_clones_repository() {
  let env = TestEnv::from_fixture("git_input.lua");

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Apply complete"));
}

#[test]
#[ignore] // Requires network access
fn git_input_resolution_in_plan() {
  let env = TestEnv::from_fixture("git_input.lua");

  env.sys_cmd().arg("plan").arg(&env.config_path).assert().success();
}
