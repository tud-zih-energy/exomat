use rstest::fixture;
use std::collections::HashMap;
use tempfile::TempDir;

use super::{
    archivist::{create_harness_dir, create_harness_file},
    fs_names::*,
};
use crate::experiment::out_file::{OutFile, OutList};
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
///         |- RUN_RUN_FILE   [EMPTY]
///         |- RUN_ENV_FILE   [EMPTY]
///         \- out_empty      [EMPTY]
/// ```
#[fixture]
pub fn skeleton_series_run() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_ENV_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join("out_empty")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         |- RUN_RUN_FILE     [EMPTY]
///         |- RUN_ENV_FILE     [EMPTY]
///         |- out_empty        [EMPTY]
///         |- out_full         ["foo bar"]
///         \- out_multi        ["foo\nbar"]
/// ```
#[fixture]
pub fn skeleton_series_run_full() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_ENV_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join("out_empty")).unwrap();
    std::fs::write(run_rep_dir.join("out_full"), "foo bar").unwrap();
    std::fs::write(run_rep_dir.join("out_multi"), "foo\nbar").unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         |- RUN_RUN_FILE     [EMPTY]
///         |- RUN_ENV_FILE     [EMPTY]
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
    std::fs::File::create(run_rep_dir.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_ENV_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join("something.txt")).unwrap();
    std::fs::File::create(run_rep_dir.join("notout_file")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     |- [TEST_RUN_REP_DIR0]/
///     |   |- RUN_RUN_FILE [EMPTY]
///     |   |- RUN_ENV_FILE [EMPTY]
///     |   \- out_empty    [EMPTY]
///     \- [TEST_RUN_REP_DIR1]/
///         |- RUN_RUN_FILE [EMPTY]
///         \- RUN_ENV_FILE [EMPTY]
/// ```
#[fixture]
pub fn filled_series_run_na() -> TempDir {
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

    std::fs::File::create(run_rep_dir_0.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir_0.join(RUN_ENV_FILE)).unwrap();
    std::fs::File::create(run_rep_dir_1.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir_1.join(RUN_ENV_FILE)).unwrap();

    std::fs::File::create(run_rep_dir_0.join("out_empty")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         |- RUN_RUN_FILE   [EMPTY]
///         |- RUN_ENV_FILE   [EMPTY]
///         |- out_some       [EMPTY]
///         \- out_some.txt   [EMPTY]
/// ```
#[fixture]
pub fn filled_series_run_duplicate() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_ENV_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join("out_some.txt")).unwrap();
    std::fs::File::create(run_rep_dir.join("out_some")).unwrap();

    series_dir
}

/// generates a tempdir with the following structure:
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]/
///         |- RUN_RUN_FILE [EMPTY]
///         |- RUN_ENV_FILE [EMPTY]
///         \- out_         [EMPTY]
/// ```
#[fixture]
pub fn filled_series_run_invalid() -> TempDir {
    let series_dir = TempDir::new().unwrap();
    let series_dir_path = series_dir.path().to_path_buf();

    let run_rep_dir = series_dir_path
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0);

    std::fs::create_dir_all(&run_rep_dir).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_RUN_FILE)).unwrap();
    std::fs::File::create(run_rep_dir.join(RUN_ENV_FILE)).unwrap();
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

/// generates an OutList with `1: ["a"]`
#[fixture]
pub fn outlist_1a() -> OutList {
    OutList::from(vec![OutFile::from("1", vec!["a".to_string()])]).unwrap()
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

/// generates an Outlist with `VAR: [""]`
#[fixture]
pub fn outlist_empty_string() -> OutList {
    OutList::from(vec![OutFile::from("VAR", vec!["".to_string()])]).unwrap()
}

/// generates an EnvList with `VAR: []`
#[fixture]
pub fn envlist_one_var_no_val() -> EnvList {
    HashMap::from([("VAR".to_string(), vec![])])
}

/// generates an OutList with `VAR: []`
#[fixture]
pub fn outlist_one_var_no_val() -> OutList {
    OutList::from(vec![OutFile::from("VAR", vec![])]).unwrap()
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

/// generates an OutList with `VAR1: ["VALUE", "baz"], VAR2: ["", "a,b"]`
#[fixture]
pub fn outlist_mixed_weird() -> OutList {
    OutList::from(vec![
        OutFile::from("VAR1", vec!["VALUE".to_string(), "baz".to_string()]),
        OutFile::from("VAR2", vec![String::new(), "a,b".to_string()]),
    ])
    .unwrap()
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

/// generates a Series dir with three Run reps and out_ files
///
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     |- [TEST_RUN_REP_DIR0]
///     |   |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///     |   |- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
///     |   |- out_number           [content: "1\n2"]
///     |   \- out_word             [content: "one\ntwo"]
///     |- [TEST_RUN_REP_DIR1]
///     |   |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///     |   |- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
///     |   |- out_number           [content: "1\n2"]
///     |   \- out_word             [content: "one\ntwo"]
///     \- [TEST_RUN_REP_DIR2]
///         |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///         |- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
///         |- out_number           [content: "1\n2"]
///         \- out_word             [content: "one\ntwo"]
/// ```
#[fixture]
pub fn setup_series_dir() -> TempDir {
    let tmp_run = TempDir::new().unwrap();
    let runs_dir = tmp_run.path().to_path_buf();

    // create run rep
    let equal_run = runs_dir.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0);
    let unequal_run = runs_dir.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR1);
    let empty_run = runs_dir.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR2);
    std::fs::create_dir_all(&equal_run).unwrap();
    std::fs::create_dir_all(&unequal_run).unwrap();
    std::fs::create_dir_all(&empty_run).unwrap();

    // Create simple run script
    std::fs::write(&unequal_run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();
    std::fs::write(&equal_run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();
    std::fs::write(&empty_run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();

    // Create env file for all runs
    std::fs::write(&unequal_run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();
    std::fs::write(&equal_run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();
    std::fs::write(&empty_run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

    // Create out_ files (equal)
    std::fs::write(&equal_run.join("out_number"), "1\n2").unwrap();
    std::fs::write(&equal_run.join("out_word"), "one\ntwo").unwrap();

    // Create out_ files (unequal)
    std::fs::write(&unequal_run.join("out_number"), "1\n2\n3").unwrap();
    std::fs::write(&unequal_run.join("out_word"), "NA").unwrap();

    // Create out_ files (empty)
    std::fs::write(&unequal_run.join("out_number"), "1\n2\n").unwrap();
    std::fs::File::create(&unequal_run.join("out_word")).unwrap();

    tmp_run
}

/// generates a Series dir with one Run rep
///
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///     \- [TEST_RUN_REP_DIR0]
///         \- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
/// ```
#[fixture]
pub fn setup_series_no_out() -> TempDir {
    let tmp_run = TempDir::new().unwrap();
    let series = tmp_run.path().to_path_buf();
    let run = series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0);
    std::fs::create_dir_all(&run).unwrap();

    // Create simple run script
    std::fs::write(&run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();

    // Create env file
    std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

    tmp_run
}

/// generates a Series dir with one Run rep and an empty out_ file
///
/// ```notest
/// tempdir/
/// \- [SERIES_RUNS_DIR]/
///     \- [TEST_RUN_REP_DIR0]
///         |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///         |- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
///         \- [out_empty]          [EMPTY]
/// ```
#[fixture]
pub fn setup_series_empty_out() -> TempDir {
    let tmp_run = TempDir::new().unwrap();
    let series = tmp_run.path().to_path_buf();
    let run = series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0);
    std::fs::create_dir_all(&run).unwrap();

    // Create simple run script
    std::fs::write(&run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();

    // Create env file
    std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

    // create empty out files
    std::fs::File::create(&run.join("out_empty")).unwrap();

    tmp_run
}

/// generates a Series dir with one Run rep and an out file that shadows an env var
///
/// ```notest
/// tempdir/
///  |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///  |- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
///  |- [out_VAR1]           [content: "1"]
///  \- [out_word]           [content: "one"]
/// ```
#[fixture]
pub fn setup_run_dir_shadow() -> TempDir {
    let tmp_run = TempDir::new().unwrap();
    let run = tmp_run.path().to_path_buf();

    // Create simple run script
    std::fs::write(&run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();

    // Create env file for both runs
    std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

    // Create out_ files (equal)
    std::fs::write(&run.join("out_VAR1"), "1").unwrap();
    std::fs::write(&run.join("out_word"), "one").unwrap();

    tmp_run
}

/// generates a Run dir with out_ files
///
/// ```notest
/// tempdir/
///  |- [RUN_RUN_FILE]       [content: "echo $VAR1"]
///  |- [RUN_ENV_FILE]       [content: "VAR1=foo\nVAR2=bar"]
///  |- [out_number]         [content: "1\n2"]
///  \- [out_word]           [content: "one\ntwo]
/// ```
#[fixture]
pub fn setup_run_dir() -> TempDir {
    let tmp_run = TempDir::new().unwrap();
    let run = tmp_run.path().to_path_buf();

    // Create simple run script
    std::fs::write(&run.join(RUN_RUN_FILE), "echo $VAR1").unwrap();

    // Create env file for both runs
    std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

    // Create out_ files (equal)
    std::fs::write(&run.join("out_number"), "1\n2").unwrap();
    std::fs::write(&run.join("out_word"), "one\ntwo").unwrap();

    tmp_run
}
