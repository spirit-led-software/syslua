//! Integration tests for syslua.pkgs.* package builds.
//!
//! These tests actually download and build packages, so they're marked #[ignore]
//! by default. Run with: cargo test -p syslua-cli --test integration pkgs -- --ignored

use predicates::prelude::*;

use super::common::TestEnv;

#[test]
#[ignore = "downloads ~2MB binary from GitHub"]
fn pkg_jq_builds_and_produces_executable() {
  let env = TestEnv::from_fixture("pkg_jq.lua");

  env
    .sys_cmd()
    .arg("apply")
    .arg(&env.config_path)
    .timeout(std::time::Duration::from_secs(120))
    .assert()
    .success()
    .stdout(predicate::str::contains("Builds realized"));
}
