//! Plan command integration tests.

use predicates::prelude::*;

use super::common::TestEnv;

#[test]
fn plan_minimal_config() {
  let env = TestEnv::from_fixture("minimal.lua");

  env
    .sys_cmd()
    .arg("plan")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds: 0"));
}

#[test]
fn plan_build_shows_count() {
  let env = TestEnv::from_fixture("build_with_exec.lua");

  env
    .sys_cmd()
    .arg("plan")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds: 1"));
}

#[test]
fn plan_multi_build_shows_count() {
  let env = TestEnv::from_fixture("multi_build.lua");

  env
    .sys_cmd()
    .arg("plan")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds: 2"));
}

#[test]
fn plan_bind_shows_count() {
  let env = TestEnv::from_fixture("bind_create.lua");

  env
    .sys_cmd()
    .arg("plan")
    .arg(&env.config_path)
    .assert()
    .success()
    .stdout(predicate::str::contains("Binds: 1"));
}
