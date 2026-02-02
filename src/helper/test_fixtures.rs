use rstest::fixture;
use std::collections::HashMap;
use tempfile::TempDir;

use super::{
    archivist::{create_harness_dir, create_harness_file},
    fs_names::*,
};
use crate::harness::env::{EnvList, Environment};

#[fixture]
pub fn skeleton_src() -> TempDir {
    tempfile::tempdir().expect("Could not create tempdir")
}

#[fixture]
pub fn skeleton_src_envs() -> TempDir {
    let dir = tempfile::tempdir().expect("Could not create tempdir");
    let dir_path = dir.path().to_path_buf();
    create_harness_dir(&dir_path.join(SRC_ENV_DIR)).unwrap();

    dir
}

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
