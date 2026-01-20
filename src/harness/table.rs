//! harness make-table command

use csv::Writer;
use itertools::Itertools;
use log::{debug, error, trace, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::harness::env::{EnvList, Environment};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Filters all "out_$NAME" files from the given experiment series directory. Then creates
/// a map with each $NAME becomming a key and the accumulated content of all
/// `series_dir/runs/run_*_rep*/out_$NAME` files becomming the associated value.
///
/// If `out_$NAME` is found in one experiment run directory, but not in another, a "NA"
/// will be added to the list of values.
///
/// The content of `out_$NAME` files is not validated or checked in any way, if you put
/// weird content in them, you will get weird output.
///
/// ## Example
/// ```
/// use exomat::harness::table::collect_output;
/// use exomat::helper::fs_names::*;
///
/// use tempfile::TempDir;
/// use std::fs::{File, create_dir_all};
/// use std::io::Write;
///
/// // create (repetition) dir
/// let series_dir = TempDir::new().unwrap();
/// let series_dir = series_dir.path().to_path_buf();
///
/// // create multiple repetition dirs
/// let run_rep_dir_0 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
/// create_dir_all(&run_rep_dir_0).unwrap();
/// let run_rep_dir_1 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep1");
/// create_dir_all(&run_rep_dir_1).unwrap();
///
/// // add multiple out_ files and some that will not be used
/// File::create(run_rep_dir_0.join("not_out_file")).unwrap();
/// File::create(run_rep_dir_0.join("random")).unwrap();
///
/// File::create(run_rep_dir_0.join("out_empty.txt")).unwrap();
/// let mut some_0 = File::create(run_rep_dir_0.join("out_some")).unwrap();
/// let mut some_1 = File::create(run_rep_dir_1.join("out_some")).unwrap();
///
/// // fill out_some
/// some_0.write_all(b"foo").unwrap();
/// some_1.write_all(b"bar").unwrap();
///
/// let res = collect_output(&series_dir).unwrap();
///
/// // check empty
/// let res_vec = res.get("empty.txt").unwrap();
/// assert!(res_vec.contains(&String::new()));      // empty string from run_rep_dir_0
/// assert!(res_vec.contains(&String::from("NA"))); // "NA" from run_rep_dir_1
///
/// // check some
/// let res_vec = res.get("some").unwrap();
/// assert!(res_vec.contains(&String::from("foo"))); // "foo" from run_rep_dir_0
/// assert!(res_vec.contains(&String::from("bar"))); // "bar" from run_rep_dir_1
/// ```
pub fn collect_output(series_dir: &Path) -> Result<HashMap<String, Vec<String>>> {
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
    // (1) Get the maximum per-dir length of a value
    let mut max_length_by_dir: HashMap<PathBuf, usize> = HashMap::new();
    for (dir, values) in value_by_var_by_dir.iter() {
        let max_len = values
            .values()
            .filter_map(|val| val.get(0))
            .map(|value| value.lines().count().max(1))
            .max()
            .unwrap_or(1);
        max_length_by_dir.insert(dir.clone(), max_len);
    }

    // (2) For every directory
    for (dir, max_length) in max_length_by_dir {
        let value_by_var = value_by_var_by_dir.get(&dir).unwrap().clone();
        for (var, vals) in value_by_var {
            if vals.len() != 1 {
                return Err(Error::EnvError { reason: format!("Input to split_and_balance_multiline must be singular value, got {} values for {}!", vals.len(), var)});
            } else {
                let value = vals.get(0).unwrap();

                // Split each value on newlines
                let mut split: Vec<String> = value.split('\n').map(|s| s.to_string()).collect();

                // Is this a single value? Then copy it max_length times to make all columns the
                // same length
                if split.len() == 1 && max_length > 1 {
                    let to_extend = max_length - split.len();
                    split.extend(vec![split[0].clone(); to_extend]);

                // We got multiple values for var, check if it has the same number of rows as the
                // other columns
                } else if split.len() != max_length {
                    return Err(Error::EnvError {
                                reason: format!("Mismatched number of values for {var} {}, other value in {} has {max_length}", split.len(), dir.display())});
                }

                // insert the balanced list for each repetition
                value_by_var_by_dir
                    .get_mut(&dir)
                    .unwrap()
                    .insert(var.clone(), split);
            }
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
pub fn serialize_csv(file: &PathBuf, content: &HashMap<String, Vec<String>>) -> Result<()> {
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
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn table_serialize_correct() {
        // create output file (empty)
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();

        let out_file_0 = tmpdir.join("0.csv");
        let out_file_1 = tmpdir.join("1.csv");
        let out_file_2 = tmpdir.join("2.csv");

        // not created yet
        assert!(!out_file_0.is_file());
        assert!(!out_file_1.is_file());
        assert!(!out_file_2.is_file());

        // keys but no values
        let content_0 = HashMap::from([("empty".to_string(), vec!["".to_string()])]);

        // one key, one value
        let content_1 = HashMap::from([("foo".to_string(), vec!["1".to_string()])]);

        // multiple keys, multiple values
        let content_2 = HashMap::from([
            (
                "bar".to_string(),
                vec!["42".to_string(), "with,comma".to_string()],
            ),
            ("baz".to_string(), vec![String::new(), "a".to_string()]),
        ]);

        serialize_csv(&out_file_0, &content_0).unwrap();
        serialize_csv(&out_file_1, &content_1).unwrap();
        serialize_csv(&out_file_2, &content_2).unwrap();

        assert_eq!(
            std::fs::read_to_string(out_file_0).unwrap(),
            String::from("empty\n\"\"\n")
        );
        assert_eq!(
            std::fs::read_to_string(out_file_1).unwrap(),
            String::from("foo\n1\n")
        );

        // with multiple keys and values the order of items after serialization is
        // random, so only check if the correct lines are there
        let file_2_string = std::fs::read_to_string(out_file_2).unwrap();

        assert!(file_2_string.contains("bar,baz\n") || file_2_string.contains("baz,bar\n"));
        assert!(file_2_string.contains("42,\n") || file_2_string.contains(",42\n"));
        assert!(
            file_2_string.contains("\"with,comma\",a\n")
                || file_2_string.contains("a,\"with,comma\"\n")
        );
    }

    #[test]
    fn table_serialize_empty() {
        // create output file (empty)
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();
        let out_file = tmpdir.join("0.csv");
        assert!(!out_file.is_file());

        let content: HashMap<String, Vec<String>> = HashMap::new();

        assert!(serialize_csv(&out_file, &content).is_ok());

        // file should be created, but remain empty
        assert!(out_file.is_file());
        assert!(std::fs::read_to_string(out_file).unwrap().is_empty());
    }

    #[test]
    fn table_collect_empty() {
        // create empty (series) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        std::fs::create_dir_all(&series_dir).unwrap();

        // test all collection funcs with empty directory
        let res = collect_output(&series_dir).unwrap();
        assert!(res.is_empty());

        let res = find_all_files(&series_dir).unwrap();
        assert!(res.is_empty());

        let res = find_all_run_repetitions(&series_dir);
        assert!(res.is_empty());
    }

    #[test]
    fn table_collect_repetition_no_out() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir).unwrap();

        // add various content, but no out_ file
        std::fs::File::create(run_rep_dir.join("something.txt")).unwrap();
        std::fs::File::create(run_rep_dir.join("notout_file")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn table_collect_empty_out() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir).unwrap();

        // add empty out_ file
        std::fs::File::create(run_rep_dir.join("out_empty")).unwrap();

        // key "empty" should be present, but without values
        let res = collect_output(&series_dir).unwrap();
        assert!(res.get("empty") == Some(&vec![String::new()]));
    }

    #[test]
    fn table_collect_no_value() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();

        // create multiple repetition dirs
        let run_rep_dir_0 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir_0).unwrap();
        let run_rep_dir_1 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep1");
        std::fs::create_dir_all(&run_rep_dir_1).unwrap();

        // add empty out_ file in one of them
        std::fs::File::create(run_rep_dir_0.join("out_empty")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        let res_vec = res.get("empty").unwrap();

        assert!(res_vec.contains(&String::new())); // empty string from run_rep_dir_0
        assert!(res_vec.contains(&String::from("NA"))); // "NA" from run_rep_dir_1
    }

    #[test]
    fn table_collect_duplicates() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir).unwrap();

        // add out files with the same name
        std::fs::File::create(run_rep_dir.join("out_some.txt")).unwrap();
        std::fs::File::create(run_rep_dir.join("out_some")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert!(res.get("some").is_some());
        assert!(res.get("some.txt").is_some());
    }

    #[test]
    fn table_collect_out_no_name() {
        // create dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir).unwrap();

        // add out file without name
        std::fs::File::create(run_rep_dir.join("out_")).unwrap();

        assert!(collect_output(&series_dir).is_err());
    }

    #[test]
    fn table_collect_multiline() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir).unwrap();

        // add out files
        let multi = run_rep_dir.join("out_multi");
        std::fs::File::create(&multi).unwrap();

        let single = run_rep_dir.join("out_single");
        std::fs::File::create(&single).unwrap();

        let trailing = run_rep_dir.join("out_trailing");
        std::fs::File::create(&trailing).unwrap();

        // write content to files
        std::fs::write(multi, "11\n20").unwrap();
        std::fs::write(trailing, "11\n20\n").unwrap();
        std::fs::write(single, "foo").unwrap();

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

    #[test]
    fn table_collect_multiline_empty() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir).unwrap();

        // add out files
        let multi = run_rep_dir.join("out_multi");
        std::fs::File::create(&multi).unwrap();

        let single = run_rep_dir.join("out_empty");
        std::fs::File::create(&single).unwrap();

        // write content to files
        std::fs::write(multi, "foo\nbar").unwrap();

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
    #[test]
    fn table_collect_multiline_mismatch() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir1 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir1).unwrap();

        // add out files in both run reps
        let out_foo = run_rep_dir1.join("out_foo");
        std::fs::File::create(&out_foo).unwrap();

        let out_bar = run_rep_dir1.join("out_bar");
        std::fs::File::create(&out_bar).unwrap();

        // write content to files
        std::fs::write(out_foo, "11\n20").unwrap(); // two lines
        std::fs::write(out_bar, "6\n48\n15").unwrap(); // three lines

        // check content
        assert!(collect_output(&series_dir).is_err());
    }
    // If there are multiple runs, then the number of rows in a value
    // can differ between
    #[test]
    fn table_collect_multiline_multiple_dirs_diff_length() {
        // create (repetition) dir
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_rep_dir1 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep0");
        std::fs::create_dir_all(&run_rep_dir1).unwrap();

        let run_rep_dir2 = series_dir.join(SERIES_RUNS_DIR).join("run_x_rep1");
        std::fs::create_dir_all(&run_rep_dir2).unwrap();

        // add out files in both run reps
        let multi1 = run_rep_dir1.join("out_foo");
        std::fs::File::create(&multi1).unwrap();

        let multi2 = run_rep_dir2.join("out_foo");
        std::fs::File::create(&multi2).unwrap();

        // write content to files
        std::fs::write(multi1, "11\n20").unwrap(); // two lines
        std::fs::write(multi2, "6\n48\n15").unwrap(); // three lines

        // check content
        assert!(collect_output(&series_dir).is_ok());
    }
}
