//! harness make-table command

use csv::Writer;
use itertools::Itertools;
use log::{debug, error, info, trace, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::harness::env::{EnvList, Environment};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Entrypoint for table binary
///
/// Filters output (files) from every run repetition in the pwd.
/// Looks through every `series_dir/runs/run_*` directory and accumulates the content of
/// every `out_*` file into one csv file.
///
/// ## Example
/// ```bash
/// exp_series
/// \-> runs
///     |-> run_0_rep0
///     |   |-> out_foo # content: "42"
///     |   \-> out_bar # content: "true"
///     \-> run_0_rep1
///         |-> out_foo # content: "300"
///         \-> out_bar # content: "false"
/// ```
/// results in `exp_series.csv` with:
/// ```notest
/// foo,bar
/// 42, true
/// 300,false
/// ```
pub fn main() -> Result<()> {
    let series_dir = crate::find_marker_pwd(MARKER_SERIES)?;

    // collect all output from every run in series_dir
    let out_content = collect_output(&series_dir)?;
    info!("Collected output for {} keys", out_content.len());
    info!("Found keys: {:?}", out_content.keys());

    // output file will be "series_dir/[series_dir].csv"
    let mut out_file = PathBuf::from(
        series_dir
            .file_name()
            .expect("Could not read experiment series name"),
    );
    out_file.set_extension("csv");

    // serialize data and write to file
    serialize_csv(&series_dir.join(out_file), &out_content)?;

    Ok(())
}

/// Filters all "out_$NAME" files from the given experiment series directory. Then creates
/// a map with each $NAME becomming a key and the accumulated content of all
/// `series_dir/runs/run_*_rep*/out_$NAME` files becomming the associated value.
///
/// If `out_$NAME` is found in one experiment run directory, but not in another, a "NA"
/// will be added to the list of values.
///
/// The content of `out_$NAME` files is not validated or checked in any way, if you put
/// weird content in them, you will get weird output.
fn collect_output(series_dir: &Path) -> Result<HashMap<String, Vec<String>>> {
    // filter all runs/run_[env]_rep[rep] from a series directory
    let runs_dir = series_dir.join(SERIES_RUNS_DIR);
    let run_repetitions = find_all_run_repetitions(&runs_dir);

    // (1) fetch vars from all experiment run directories
    let mut value_by_var_by_dir: HashMap<PathBuf, EnvList> = HashMap::new();
    for repetition_dir in &run_repetitions {
        debug!("fetching vars from: {}", repetition_dir.display());

        // (1a) initialize with content from env
        let env_file = repetition_dir.join(RUN_ENV_FILE);
        let mut value_by_var = Environment::from_file(&env_file).unwrap_or_else(|err| {
            error!(
                "could not load environment variables from {RUN_ENV_FILE} in {}: {err}",
                repetition_dir.display()
            );
            Environment::new()
        });

        // (1b) insert content from out_ files
        let prefix = "out_";
        let contained_files = find_all_files(repetition_dir)?;
        for file in contained_files.iter().filter_map(|file| {
            file.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| name.starts_with(prefix))
                .map(|_| file)
        }) {
            let var_name = file_name_string(file)
                .strip_prefix(prefix)
                .unwrap()
                .to_string();
            if var_name.is_empty() {
                return Err(Error::Empty(
                    "variable name (prefix out_ alone is not permitted)".to_string(),
                ));
            }
            if value_by_var.contains_env_var(&var_name) {
                warn!(
                    "in {}: out_{var_name} shadows input environment variable ${var_name}",
                    repetition_dir.display()
                );
            }

            // may contain line breaks, is handled later
            value_by_var.add_env(var_name, std::fs::read_to_string(file)?.trim().to_string());
        }

        value_by_var_by_dir.insert(repetition_dir.to_path_buf(), value_by_var.to_env_list());
    }

    // (2) transform to correct output type
    split_and_balance_multiline(&mut value_by_var_by_dir)?;
    let mut values_by_var: HashMap<String, Vec<String>> = HashMap::new();

    // (2a) collect all var names
    for (dir, value_by_var) in &value_by_var_by_dir {
        for var in value_by_var.keys() {
            if !values_by_var.contains_key(var) {
                trace!("adding key to output from {}: {var}", dir.display());
                values_by_var.insert(var.clone(), Vec::new());
            }
        }
    }

    // (2b) populate content for each var
    for (dir, value_by_var) in &value_by_var_by_dir {
        for (var, values) in values_by_var.iter_mut() {
            values.extend(match value_by_var.get(var) {
                None => {
                    warn!(
                        "experiment in {} misses value for variable: {var}",
                        dir.display()
                    );
                    vec!["NA".to_string()]
                }
                Some(x) => x.clone(),
            });
        }
    }

    Ok(values_by_var)
}

/// Adds each line as a separate value, while keeping the number of values even
/// across all dirs.
///
/// ## Example
/// ```notest
/// value_by_var_by_dir = rep1: [
///                             "var1" = ["foo", "bar\nbaz"],
///                             "var2" = ["12"],
///                             "var3" = ["a", "b"]
///                             ],
///                       rep2: [
///                             "var1" = ["FOO", "BAR\nBAZ"],
///                             "var2" = ["22"],
///                             "var3" = ["b", "a"]
///                             ]
/// ```
///
/// turns into
/// ```notest
/// value_by_var_by_dir = rep1: [
///                             "var1" = ["foo", "bar", "baz"],
///                             "var2" = ["12", "12", "12"],
///                             "var3" = ["a", "b", "b"]
///                             ],
///                       rep2: [
///                             "var1" = ["FOO", "BAR", "BAZ"],
///                             "var2" = ["22", "22", "22"],
///                             "var3" = ["b", "a", "a"]
///                             ]
/// ```
///
/// ## Errors and Panics
/// - Returns an `EnvError` if  the var=value pairs put in dont have a single item (we split it
/// here).
/// - Returns an `EnvError` if there are two values in the same dir with different numbers of rows.
/// - Panics if the maximum amount of values cannot be determined for a variable.
fn split_and_balance_multiline(value_by_var_by_dir: &mut HashMap<PathBuf, EnvList>) -> Result<()> {
    // For every directory
    for (dir, value_by_var) in value_by_var_by_dir {
        // Get the maximum per-dir length of a value
        let max_length = value_by_var
            .values()
            .filter_map(|val| val.get(0))
            .map(|value| value.lines().count().max(1))
            .max()
            .unwrap_or(1);

        // for each variable
        for (var, vals) in value_by_var.iter_mut() {
            if vals.len() != 1 {
                return Err(Error::EnvError { reason: format!("Input to split_and_balance_multiline must be singular value, got {} values for {}!", vals.len(), var)});
            }

            let value = vals.first().unwrap();
            let mut split: Vec<String> = value.split('\n').map(|s| s.to_string()).collect();

            // Is this a single value? Then copy it max_length times to make all columns the
            // same length
            if split.len() == 1 && max_length > 1 {
                // Cannot use Vec::repeat() here, because String does not implement the Copy Trait >:(
                let to_extend = max_length - split.len();
                split.extend(vec![split.first().unwrap().clone(); to_extend]);

            // We got multiple values for var, check if it has the same number of rows as the
            // other columns
            } else if split.len() != max_length {
                return Err(Error::EnvError {
                                reason: format!("Mismatched number of values for {var} {}, other value in {} has {max_length}", split.len(), dir.display())});
            }

            // Update the value list
            *vals = split;
        }
    }

    Ok(())
}

/// Builds and returns a vector of all readable files in the given directory.
///
/// ## Panics
/// - Panics if directory traversal went wrong
fn find_all_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::<PathBuf>::new();

    for entry in dir.read_dir().expect("Could not read dir") {
        if entry
            .as_ref()
            .expect("Entry not readable")
            .metadata()
            .expect("Metadata of entry not readable")
            .is_file()
        {
            files.push(entry.unwrap().path());
        }
    }

    Ok(files)
}

/// Builds and returns a vector of all run repetitions in the given directory.
///
/// A directory is considered a run repetition, if it's name starts with "run_".
///
/// ## Panics
/// - Panics if directory traversal went wrong
fn find_all_run_repetitions(runs_dir: &Path) -> Vec<PathBuf> {
    let mut repetitions = Vec::<PathBuf>::new();

    // return the empty vector if runs_dir does not exist
    if !runs_dir.is_dir() {
        return repetitions;
    }

    for entry in runs_dir.read_dir().expect("Could not read dir") {
        if entry
            .as_ref()
            .expect("Entry not readable")
            .metadata()
            .expect("Metadata of entry not readable")
            .is_dir()
        {
            // if directory name starts with "run_", it is considered a run repetition
            if entry
                .as_ref()
                .unwrap()
                .path() // complete path
                .file_name() // last part of path; directory name
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("run_")
            {
                repetitions.push(entry.unwrap().path());
            }
        }
    }

    repetitions
}

/// Takes a Hashmap and serializes it's content into `file`.
///
/// Requires all values in `content` to be of equal length. If `content` is empty,
/// `file` will still be created.
///
/// Uses the default CSV delimiter `,`. Any values containing it will be escaped using
/// `""`.
///
/// ## Errors and Panics
/// - Panics if not all values of `content` have the same number of elements
/// - Returns a `CsvError` if something went wrong during the csv serialization
fn serialize_csv(file: &PathBuf, content: &HashMap<String, Vec<String>>) -> Result<()> {
    // assert all values have the same number of elements
    assert!(
        content.values().map(|v| v.len()).all_equal(),
        "Content has unequal amount of values: {content:?}"
    );

    let mut wtr = Writer::from_path(file).map_err(|e| Error::CsvError {
        reason: e.to_string(),
    })?;

    // only try to write something if content is not empty, else simply flush and exit
    if !content.is_empty() {
        // write header
        let keys: Vec<&String> = content.keys().collect();
        wtr.write_record(keys).map_err(|e| Error::CsvError {
            reason: e.to_string(),
        })?;

        let val_len = content.values().map(|v| v.len()).max().unwrap();

        //write content
        for i in 0..val_len {
            // write ith element of each Vector
            let row: Vec<String> = content
                .keys()
                .map(|key| {
                    content
                        .get(key)
                        .and_then(|values| values.get(i))
                        .expect("Could not access value")
                        .clone()
                })
                .collect();

            wtr.write_record(row).map_err(|e| Error::CsvError {
                reason: e.to_string(),
            })?;
        }
    }

    wtr.flush().map_err(|e| Error::CsvError {
        reason: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use tempfile::TempDir;

    use super::*;
    use crate::helper::test_fixtures::{
        envlist_1a, envlist_empty_string, envlist_mixed_weird, envlist_one_var_no_val,
        filled_series_run_NA, filled_series_run_duplicate, filled_series_run_invalid,
        skeleton_series_run, skeleton_series_run_empty, skeleton_src,
    };
    use crate::helper::test_helper::{contains_either, create_out_file};

    #[rstest]
    fn table_serialize_multiline(
        #[from(skeleton_src)] outdir: TempDir,
        envlist_mixed_weird: EnvList,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("2.csv");

        // not created yet
        assert!(!out_file.is_file());

        serialize_csv(&out_file, &envlist_mixed_weird).unwrap();

        // with multiple keys and values the order of items after serialization is
        // random, so only check if the correct lines are there
        let file_2 = std::fs::read_to_string(out_file).unwrap();
        assert!(contains_either(&file_2, "VAR1,VAR2\n", "VAR2,VAR1\n"));
        assert!(contains_either(&file_2, "VALUE,\n", ",VALUE\n"));
        assert!(contains_either(&file_2, "\"a,b\",baz\n", "baz,\"a,b\"\n"));
    }

    #[rstest]
    #[case(HashMap::new(), "")]
    #[case(envlist_1a(), "1\na\n")]
    #[case(envlist_one_var_no_val(), "VAR\n")]
    #[case(envlist_empty_string(), "VAR\n\"\"\n")]
    fn table_serialize_single(
        #[from(skeleton_src)] outdir: TempDir,
        #[case] envlist: EnvList,
        #[case] expected: String,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("0.csv");

        // not created yet
        assert!(!out_file.is_file());

        serialize_csv(&out_file, &envlist).unwrap();

        assert_eq!(std::fs::read_to_string(out_file).unwrap(), expected);
    }

    #[rstest]
    fn table_collect_empty(#[from(skeleton_src)] series_dir: TempDir) {
        let series_dir = series_dir.path().to_path_buf();

        // test all collection funcs with empty directory
        let res = collect_output(&series_dir).unwrap();
        assert!(res.is_empty());

        let res = find_all_files(&series_dir).unwrap();
        assert!(res.is_empty());

        let res = find_all_run_repetitions(&series_dir);
        assert!(res.is_empty());
    }

    #[rstest]
    fn table_collect_repetition_no_out(skeleton_series_run_empty: TempDir) {
        let series_dir = skeleton_series_run_empty.path().to_path_buf();

        let res = collect_output(&series_dir).unwrap();
        assert!(res.is_empty());
    }

    #[rstest]
    fn table_collect_empty_out(skeleton_series_run: TempDir) {
        let series_dir = skeleton_series_run.path().to_path_buf();

        // key "empty" should be present, but without values
        let res = collect_output(&series_dir).unwrap();
        assert!(res.get("empty") == Some(&vec![String::new()]));
    }

    #[rstest]
    #[allow(non_snake_case)]
    fn table_collect_no_value(filled_series_run_NA: TempDir) {
        let series_dir = filled_series_run_NA.path().to_path_buf();

        let res = collect_output(&series_dir).unwrap();
        let res_vec = res.get("empty").unwrap();

        assert!(res_vec.contains(&String::new())); // empty string from run_rep_dir_0
        assert!(res_vec.contains(&String::from("NA"))); // "NA" from run_rep_dir_1
    }

    #[rstest]
    fn table_collect_duplicates(filled_series_run_duplicate: TempDir) {
        let series_dir = filled_series_run_duplicate.path().to_path_buf();

        let res = collect_output(&series_dir).unwrap();
        assert!(res.get("some").is_some());
        assert!(res.get("some.txt").is_some());
    }

    #[rstest]
    fn table_collect_out_no_name(filled_series_run_invalid: TempDir) {
        let series_dir = filled_series_run_invalid.path().to_path_buf();

        assert!(collect_output(&series_dir).is_err());
    }

    #[rstest]
    fn table_collect_multiline(skeleton_series_run: TempDir) {
        let series_dir = skeleton_series_run.path().to_path_buf();

        // add out files
        create_out_file(&series_dir, None, "out_single", "foo");
        create_out_file(&series_dir, None, "out_multi", "11\n20");
        create_out_file(&series_dir, None, "out_trailing", "11\n20");

        // check content, order is important
        let res = collect_output(&series_dir).unwrap();
        assert!(res.get("multi").is_some());
        assert_eq!(
            res.get("multi").unwrap(),
            &vec!["11".to_string(), "20".to_string()]
        );

        // same as multi
        assert!(res.get("trailing").is_some());
        assert_eq!(
            res.get("trailing").unwrap(),
            &vec!["11".to_string(), "20".to_string()]
        );

        assert!(res.get("single").is_some());
        assert_eq!(
            res.get("single").unwrap(),
            &vec!["foo".to_string(), "foo".to_string()]
        );
    }

    #[rstest]
    fn table_collect_multiline_empty(skeleton_series_run: TempDir) {
        let series_dir = skeleton_series_run.path().to_path_buf();

        // add out files
        create_out_file(&series_dir, None, "out_multi", "foo\nbar");
        create_out_file(&series_dir, None, "out_empty", "");

        // check content
        let res = collect_output(&series_dir).unwrap();
        assert!(res.get("multi").is_some());
        assert_eq!(
            res.get("multi").unwrap(),
            &vec!["foo".to_string(), "bar".to_string()]
        );

        assert!(res.get("empty").is_some());
        assert_eq!(
            res.get("empty").unwrap(),
            &vec![String::new(), String::new()]
        );
    }

    // If there are two values in the same run,
    // they have to have the same number of rows.
    #[rstest]
    fn table_collect_multiline_mismatch(skeleton_series_run: TempDir) {
        let series_dir = skeleton_series_run.path().to_path_buf();

        // add out files in both run reps
        create_out_file(&series_dir, None, "out_foo", "11\n20"); // two lines
        create_out_file(&series_dir, None, "out_bar", "6\n48\n15"); // three lines

        // check content
        assert!(collect_output(&series_dir).is_err());
    }

    // If there are multiple runs, then the number of rows in a value
    // can differ between
    #[rstest]
    #[allow(non_snake_case)]
    fn table_collect_multiline_multiple_dirs_diff_length(filled_series_run_NA: TempDir) {
        let series_dir = filled_series_run_NA.path().to_path_buf();

        // add out files in both run reps
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_foo", "11\n20"); // two lines
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR1), "out_foo", "6\n48\n15"); // three lines

        // check content
        assert!(collect_output(&series_dir).is_ok());
    }

    #[rstest]
    #[allow(non_snake_case)]
    fn table_collect_output_full(filled_series_run_NA: TempDir) {
        let series_dir = filled_series_run_NA.path().to_path_buf();

        // add multiple out_ files and some that will not be used
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "not_out_file", "");
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "random", "");

        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_empty.txt", "");
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_some", "foo");
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR1), "out_some", "bar");

        let res = collect_output(&series_dir).unwrap();

        // check empty
        let res_vec = res.get("empty.txt").unwrap();
        assert!(res_vec.contains(&String::new())); // empty string from run_rep_dir_0
        assert!(res_vec.contains(&String::from("NA"))); // "NA" from run_rep_dir_1

        // check some
        let res_vec = res.get("some").unwrap();
        assert!(res_vec.contains(&String::from("foo"))); // "foo" from run_rep_dir_0
        assert!(res_vec.contains(&String::from("bar"))); // "bar" from run_rep_dir_1
    }
}
