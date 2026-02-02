use rstest::fixture;
use std::collections::HashMap;
use tempfile::TempDir;

use super::{archivist::create_harness_dir, fs_names::*};
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
