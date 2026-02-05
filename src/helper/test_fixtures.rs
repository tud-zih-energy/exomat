use rstest::fixture;
use std::collections::HashMap;
use tempfile::TempDir;

use super::{
    archivist::{create_harness_dir, create_harness_file},
    fs_names::*,
};
use crate::harness::env::{EnvList, Environment};

/// generates an empty tempdir, that can be used as an empty Experiment Source Directory
#[fixture]
pub fn skeleton_src() -> TempDir {
    tempfile::tempdir().expect("Could not create tempdir")
}

/// generates a tempdir, containing an empty subdirectory SRC_ENV_DIR
#[fixture]
pub fn skeleton_src_envs() -> TempDir {
    let dir = tempfile::tempdir().expect("Could not create tempdir");
    let dir_path = dir.path().to_path_buf();
    create_harness_dir(&dir_path.join(SRC_ENV_DIR)).unwrap();

    dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// |- [MARKER_SRC]     [EMPTY]
/// \- [SRC_ENV_DIR]/
/// ```
#[fixture]
pub fn skeleton_out() -> TempDir {
    let dir = tempfile::tempdir().expect("Could not create tempdir");
    let dir_path = dir.path().to_path_buf();

    create_harness_dir(&dir_path.join(SRC_ENV_DIR)).unwrap();
    create_harness_file(&dir_path.join(MARKER_SRC)).unwrap();

    dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         \- out_empty    [EMPTY]
/// ```
#[fixture]
pub fn skeleton_series_run() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join("out_empty")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         |- noout_file       [EMPTY]
///         \- something.txt    [EMPTY]
/// ```
#[fixture]
pub fn skeleton_series_run_empty() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    // create multiple files, but no output file
    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join("something.txt")).unwrap();
    std::fs::File::create(run_rep_dir.join("notout_file")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     |- [TEST_RUN_REP_DIR0]/
///     |   \- out_empty    [EMPTY]
///     \- [TEST_RUN_REP_DIR1]/
/// ```
#[fixture]
#[allow(non_snake_case)]
pub fn filled_series_run_NA() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    // create multiple repetition dirs
    let run_rep_dir_0 = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);
    let run_rep_dir_1 = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR1);
    std::fs::create_dir_all(&run_rep_dir_0).unwrap();
    std::fs::create_dir_all(&run_rep_dir_1).unwrap();

    std::fs::File::create(run_rep_dir_0.join("out_empty")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         |- out_some       [EMPTY]
///         \- out_some.txt    [EMPTY]
/// ```
#[fixture]
pub fn filled_series_run_duplicate() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join("out_some.txt")).unwrap();
    std::fs::File::create(run_rep_dir.join("out_some")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         \- out_       [EMPTY]
/// ```
#[fixture]
pub fn filled_series_run_invalid() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join("out_")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SRC_ENV_DIR]/
///     |- 42.env       [EMPTY]
///     |- foo.env      [EMPTY]
///     |- not_an_env   [EMPTY]
///     \- not_a_file/
/// ```
#[fixture]
pub fn filled_src_envs() -> TempDir {
    let env_dir = skeleton_src_envs();
    let env_dir_path = env_dir.path().to_path_buf();

    create_harness_file(&env_dir_path.join("42.env")).unwrap();
    create_harness_file(&env_dir_path.join("foo.env")).unwrap();
    create_harness_file(&env_dir_path.join("not_an_env")).unwrap();
    create_harness_dir(&env_dir_path.join("not_a_file")).unwrap();

    env_dir
}

/// generates an Environment with `1: "a"`
#[fixture]
pub fn env_1a() -> Environment {
    Environment::from_env_list(vec![("1".to_string(), "a".to_string())])
}

/// generates an EnvList with `1: ["a"]`
#[fixture]
pub fn envlist_1a() -> EnvList {
    HashMap::from([("1".to_string(), vec!["a".to_string()])])
}

/// generates an EnvList with `2: ["b"]`
#[fixture]
pub fn envlist_2b() -> EnvList {
    HashMap::from([("2".to_string(), vec!["b".to_string()])])
}

/// generates an Envlist with `VAR: [""]`
#[fixture]
pub fn envlist_empty_string() -> EnvList {
    HashMap::from([("VAR".to_string(), vec!["".to_string()])])
}

/// generates an EnvList with `VAR: []`
#[fixture]
pub fn envlist_one_var_no_val() -> EnvList {
    HashMap::from([("VAR".to_string(), vec![])])
}

/// generates an EnvList with `VAR: ["VAL"]`
#[fixture]
pub fn envlist_one_var_one_val() -> EnvList {
    HashMap::from([("VAR".to_string(), vec!["VAL".to_string()])])
}

/// generates an EnvList with `VAR: ["VAL", "VAL2"]`
#[fixture]
pub fn envlist_one_var_two_val() -> EnvList {
    HashMap::from([(
        "VAR".to_string(),
        vec!["VAL".to_string(), "VAL2".to_string()],
    )])
}

/// generates an EnvList with `VAR1: ["VAL1", "VAL11"], VAR2: ["VAL2", "VAL22"]`
#[fixture]
pub fn envlist_two_var_two_val() -> EnvList {
    HashMap::from([
        (
            "VAR1".to_string(),
            vec!["VAL1".to_string(), "VAL11".to_string()],
        ),
        (
            "VAR2".to_string(),
            vec!["VAL2".to_string(), "VAL22".to_string()],
        ),
    ])
}

/// generates an EnvList with `VAR1: ["VALUE"], VAR2: []`
#[fixture]
pub fn envlist_mixed() -> EnvList {
    HashMap::from([
        ("VAR1".to_string(), vec!["VALUE".to_string()]),
        ("VAR2".to_string(), vec![]),
    ])
}

/// generates an EnvList with `VAR1: ["VALUE", "baz"], VAR2: ["", "a,b"]`
#[fixture]
pub fn envlist_mixed_weird() -> EnvList {
    HashMap::from([
        (
            "VAR1".to_string(),
            vec!["VALUE".to_string(), "baz".to_string()],
        ),
        ("VAR2".to_string(), vec![String::new(), "a,b".to_string()]),
    ])
}

/// generates a Vector with `[A, B]`
#[fixture]
pub fn vec_ab() -> Vec<String> {
    vec!["A".to_string(), "B".to_string()]
}

/// generates a Vector with `[3, 2, 1]`
#[fixture]
pub fn vec_321() -> Vec<String> {
    vec!["3".to_string(), "2".to_string(), "1".to_string()]
}

/// generates a Vector with `[[VAR1, A, B], [VAR2, 3, 2, 1]]`
#[fixture]
pub fn envlist_ab321() -> Vec<Vec<String>> {
    vec![
        vec!["VAR1".to_string(), "A".to_string(), "B".to_string()],
        vec![
            "VAR2".to_string(),
            "3".to_string(),
            "2".to_string(),
            "1".to_string(),
        ],
    ]
}

use crate::harness::env::EnvironmentContainer;

/// generates an Environemnt Container with `VAR: single`
#[fixture]
pub fn container_single() -> EnvironmentContainer {
    EnvironmentContainer::from_env_list(vec![Environment::from_env_list(vec![(
        "VAR".to_string(),
        "single".to_string(),
    )])])
}

/// generates an Environemnt Container with `VAR1: VAL1, VAR2: VAL2`
#[fixture]
pub fn container_multiple() -> EnvironmentContainer {
    EnvironmentContainer::from_env_list(vec![Environment::from_env_list(vec![
        ("VAR1".to_string(), "VAL1".to_string()),
        ("VAR2".to_string(), "VAL2".to_string()),
    ])])
}
