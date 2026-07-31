use assert_cmd::pkg_name;
use predicates::prelude::*;

#[test]
fn smoketest_make_table() {
    let workspace = tempfile::tempdir().unwrap();
    std::env::set_current_dir(workspace.path()).unwrap();

    const VAR_NAME: &'static str = "VAR_SENTINEL_XHAJWSD";

    assert_cmd::Command::cargo_bin(pkg_name!())
        .unwrap()
        .args(&["skeleton", "exp_dir"])
        .assert()
        .success();

    assert_cmd::Command::cargo_bin(pkg_name!())
        .unwrap()
        .args(&["-C", "exp_dir", "env", "--add", VAR_NAME, "1", "2", "3"])
        .assert()
        .success();

    assert_cmd::Command::cargo_bin(pkg_name!())
        .unwrap()
        .args(&["run", "-o", "exp_out", "exp_dir"])
        .assert()
        .success();

    assert_cmd::Command::cargo_bin(pkg_name!())
        .unwrap()
        .args(&["-C", "exp_out", "make-table"])
        .assert()
        .success()
        .stdout(predicate::str::contains(VAR_NAME));
}
