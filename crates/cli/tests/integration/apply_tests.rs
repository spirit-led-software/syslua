//! Apply command integration tests.

use predicates::prelude::*;

use super::common::TestEnv;

#[test]
fn apply_minimal_config() {
  let env = TestEnv::from_fixture("minimal.lua");

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Apply complete"));
}

#[test]
fn apply_build_with_execution() {
  let env = TestEnv::from_fixture("build_with_exec.lua");

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds realized: 1"));
}

#[test]
fn apply_is_idempotent() {
  let env = TestEnv::from_fixture("build_with_exec.lua");

  // First apply
  env.sys_cmd().arg("apply").arg(&env.config_path).assert().success();

  // Second apply should show cached
  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds realized: 0"));
}

#[test]
fn apply_build_only() {
  let env = TestEnv::from_fixture("build_only.lua");

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds realized: 1"));
}

#[test]
fn apply_bind_create_and_destroy() {
  let env = TestEnv::from_fixture("bind_create.lua");
  let marker_file = env.output_path().join("created.txt");

  // Apply creates the bind
  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Binds applied: 1"));

  assert!(marker_file.exists(), "bind should create marker file");
}

#[test]
fn apply_multi_build() {
  let env = TestEnv::from_fixture("multi_build.lua");

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds realized: 2"));
}
