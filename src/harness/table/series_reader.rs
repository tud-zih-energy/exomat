use super::run_reader::RunReader;
use crate::harness::table::OutList;
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

use csv::Writer;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Container for an Experiment Series
#[derive(Debug, Clone)]
pub struct SeriesReader {
    runs: Vec<RunReader>,
    stdout_log: Option<String>,
    stderr_log: Option<String>,
    exomat_log: Option<String>,
}

impl SeriesReader {
    /// Immutable iteration
    pub fn iter(&self) -> SeriesReaderIter {
        SeriesReaderIter {
            series_reader: self,
            index: 0,
        }
    }

    /// Parses an Experiment Series into a SeriesReader object.
    ///
    /// If `out_$NAME` is found in one experiment run directory, but not in another, a "NA"
    /// will be added to the list of values.
    ///
    /// ### Error
    /// - Returns a `ReaderError` if any RunReader failed to parse
    pub fn parse(series: &PathBuf) -> Result<Self> {
        // find all run dirs
        let runs: Vec<RunReader> = find_run_repetitions(&series.join(SERIES_RUNS_DIR))
            .iter()
            .map(|run| {
                RunReader::parse(run).map_err(|e| Error::ReaderError {
                    dir: run.display().to_string(),
                    reason: e.to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // read log files
        let stdout_log = Self::read_log(&series.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG));
        let stderr_log = Self::read_log(&series.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG));
        let exomat_log = Self::read_log(&series.join(SERIES_RUNS_DIR).join(SERIES_EXOMAT_LOG));

        let mut reader = SeriesReader {
            runs,
            stdout_log,
            stderr_log,
            exomat_log,
        };

        reader.fill_missing_keys();
        Ok(reader)
    }

    /// Returns the list of Experiment Runs.
    pub fn get_runs(&self) -> &Vec<RunReader> {
        &self.runs
    }

    /// Serializes it's content into `file`.
    ///
    /// If the no runs are found or all runs are empty, `file` will still be created.
    ///
    /// Uses the default CSV delimiter `,`. Any values containing it will be escaped using
    /// `""`.
    ///
    /// ## Errors
    /// - Returns a `CsvError` if something went wrong during the csv serialization
    pub fn to_csv(&self, file: &PathBuf) -> Result<()> {
        let mut wtr = Writer::from_path(file).map_err(|e| Error::CsvError {
            reason: e.to_string(),
        })?;

        if !self.runs_are_empty() {
            // turn self.runs into csv rows (contains header)
            let content = self.to_csv_rows();

            for row in content {
                wtr.write_record(row).map_err(|e| Error::CsvError {
                    reason: e.to_string(),
                })?;
            }
        }

        wtr.flush().map_err(|e| Error::CsvError {
            reason: e.to_string(),
        })
    }

    /// Returns a list of all keys present in the recorded RunReader in an arbitrary order.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .runs
            .iter()
            .filter_map(|run| run.get_out_files().as_ref())
            .flat_map(|out| out.keys().map(|k| k.as_str()))
            .collect();

        // remove duplicate keys
        keys.sort();
        keys.dedup();
        keys
    }

    /// Creates a ready-to-print String based on the given parameters.
    ///
    /// ## Example
    /// Given the values:
    /// - `exp_name = Foo`
    /// - `run = Ok(_)`
    ///
    /// ```bash
    /// [Foo] exomat:
    /// [info] ...
    /// ---
    /// [Foo] stdout:
    /// normal output
    /// ---
    /// [Foo] stderr:
    ///
    /// ---
    /// [Foo] returned:
    /// Successful
    /// ```
    ///
    /// An extra "\n" will be added to `stdout`, `stderr` and `exomat`.
    ///
    /// If `run = Err(e)`, the last lines will be:
    /// ```bash
    /// [Foo] returned:
    /// Failed (reason: e)
    /// ```
    pub fn print_report<T>(&self, exp_name: &str, run: &Result<T>) {
        let mut eval_str = String::new();

        let exomat = self.exomat_log.as_deref().unwrap_or("");
        let stderr = self.stderr_log.as_deref().unwrap_or("");
        let stdout = self.stdout_log.as_deref().unwrap_or("");

        // append exomat
        eval_str.push_str(&format!("[{exp_name}] exomat:\n"));
        eval_str.push_str(exomat);
        eval_str.push_str("\n---\n");

        // append stdout
        eval_str.push_str(&format!("[{exp_name}] stdout:\n"));
        eval_str.push_str(stdout);
        eval_str.push_str("\n---\n");

        // append stderr
        eval_str.push_str(&format!("[{exp_name}] stderr:\n"));
        eval_str.push_str(stderr);
        eval_str.push_str("\n---\n");

        if self.runs_are_empty() {
            eval_str.push_str("[{exp_name}] created no output files\n")
        } else {
            if let Some(outfiles) = self.get_runs()[0].get_out_files() {
                for (out_file, content) in outfiles {
                    if content.len() > 5 {
                        // truncate after 5 lines
                        let size = content.len() - 5;
                        let (cut_content, _) = content.split_at(5);

                        eval_str.push_str(&format!(
                            "[{exp_name}] {out_file}: {cut_content:?} (...{size} more items)\n",
                        ));
                    } else if content.len() == 1 {
                        // print without brackets if only 1 element
                        eval_str
                            .push_str(&format!("[{exp_name}] {out_file}: \"{}\"\n", content[0]));
                    } else {
                        // print entire content if less than 5 lines
                        eval_str.push_str(&format!("[{exp_name}] {out_file}: {content:?}\n"));
                    }
                }
            } else {
                eval_str.push_str("[{exp_name}] error reading output files\n")
            }
        }
        eval_str.push_str("---\n");

        // append overall success/failure report
        eval_str.push_str(&format!("[{exp_name}] returned:\n"));
        match run {
            Ok(_) => eval_str.push_str("Successful\n"),
            Err(e) => eval_str.push_str(&format!("Failed (reason: {e})\n")),
        }

        print!("{eval_str}");
    }

    /// Checks if the SeriesReader contains a valid trial run.
    ///
    /// Currently checks:
    /// - run repetitions == 1
    pub fn is_valid_trial(&self) -> bool {
        if self.run_count() == 1 {
            true
        } else {
            false
        }
    }

    /// Adds missing out_ files to each RunReader.
    ///
    /// If a key is present in one RunReader but missing another, the key will be
    /// added with "NA" as it's value.
    fn fill_missing_keys(&mut self) {
        // add "NA" if a run is missing a key
        let keys: Vec<String> = self.keys().into_iter().map(|k| k.to_string()).collect();

        for run in self.runs.iter_mut() {
            for key in &keys {
                if run.get_var(key).is_none() {
                    let mut new_run = match &run.get_out_files() {
                        None => HashMap::new(),
                        Some(r) => r.clone(),
                    };
                    new_run.insert(key.clone(), vec!["NA".to_string()]);

                    run.replace_out_files_unchecked(Some(new_run));
                }
            }
        }
    }

    /// Parses `self.runs` into rows, that can be serialized in a CSV format.
    /// Includes a header row, containing `self.keys()`.
    ///
    /// Returns a Vector of all rows, with each entry being listed as a separate String.
    /// For example:
    /// ```csv
    /// word,number,comment
    /// one,1,the first number
    /// fortytwo,42,the best number
    /// ```
    ///
    /// would be represented as
    /// ```notest
    /// [
    ///     ["word", "number", "comment"],
    ///     ["one", "1", "the first number"],
    ///     ["fortytwo", "42", "the best number"]
    /// ]
    /// ```
    fn to_csv_rows(&self) -> Vec<Vec<String>> {
        // collect all rows as HashMap
        let mut rows: OutList = HashMap::new();
        for run in &self.runs {
            if let Some(out) = &run.get_out_files() {
                rows.extend(out.clone())
            } else {
                rows.extend(HashMap::new())
            }
        }

        // collect all header
        let mut rows_vec: Vec<Vec<String>> =
            vec![self.keys().iter().map(|k| k.to_string()).collect()];

        // turn all data into one list
        let val_len = rows.values().map(|v| v.len()).max().unwrap_or(0);

        for i in 0..val_len {
            // (one entry = every ith element of each key)
            let mut row: Vec<String> = Vec::new();

            for key in self.keys() {
                let value = rows.get(key).unwrap();
                row.push(value.get(i).cloned().unwrap_or_else(|| String::new()));
            }

            rows_vec.push(row);
        }

        rows_vec
    }

    /// Checks if there is anything recorded in self.runs
    ///
    /// Returns `true` if:
    /// - there are no runs
    /// - there are runs, but none contain out_ files
    /// - there are runs with out_ files, but all out_ files are empty
    fn runs_are_empty(&self) -> bool {
        if self.runs.is_empty()
            || self.runs.iter().all(|run| run.get_out_files().is_none())
            || self
                .runs
                .iter()
                .all(|run| run.get_out_files().iter().all(|out| out.is_empty()))
        {
            true
        } else {
            false
        }
    }

    /// Returns the number of runs recorded (Test helper)
    fn run_count(&self) -> usize {
        self.runs.len()
    }

    /// Parses a SeriesReader from multiple OutLists (Test helper)
    ///
    /// One OutList represents the out_files of one RunReader.
    #[cfg(test)]
    fn from_out_lists(list_of_envlist: Vec<OutList>) -> Self {
        let runs: Vec<RunReader> = list_of_envlist
            .iter()
            .map(|envlist| RunReader::from_out_list_unchecked(&envlist))
            .collect();

        SeriesReader {
            runs: runs,
            stdout_log: None,
            stderr_log: None,
            exomat_log: None,
        }
    }

    /// helper, that returns the content of a file if it is readable.
    /// Otherwise returns `None`
    fn read_log(path: &PathBuf) -> Option<String> {
        match read_to_string(path) {
            Ok(log) => Some(log),
            Err(_) => None,
        }
    }
}

// ========================== Iterator ==========================

pub struct SeriesReaderIter<'a> {
    series_reader: &'a SeriesReader,
    index: usize,
}

impl<'a> Iterator for SeriesReaderIter<'a> {
    type Item = RunReader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.series_reader.runs.len() {
            let run = self.series_reader.runs[self.index].clone();
            self.index += 1;

            Some(run)
        } else {
            None
        }
    }
}
impl<'a> IntoIterator for &'a SeriesReader {
    type Item = RunReader;
    type IntoIter = SeriesReaderIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ========================== Helper ==========================

/// Builds and returns a vector of all run repetitions in the given directory.
///
/// A directory is considered a run repetition, if it's name starts with "run_".
///
/// ## Panics
/// - Panics if directory traversal went wrong
fn find_run_repetitions(runs_dir: &Path) -> Vec<PathBuf> {
    let mut repetitions = Vec::<PathBuf>::new();

    // return the empty vector if runs_dir does not exist
    if !runs_dir.is_dir() {
        println!("runs dir empty");
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
                println!("found run: {}", entry.as_ref().unwrap().path().display());
                repetitions.push(entry.unwrap().path());
            }
        }
    }

    repetitions
}

// ========================== Tests ==========================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::helper::test_fixtures::{
        envlist_1a, envlist_empty_string, envlist_mixed_weird, envlist_one_var_no_val,
        filled_series_run_duplicate, filled_series_run_invalid, filled_series_run_na,
        setup_series_dir, setup_series_empty_out, setup_series_no_out, skeleton_series_run,
        skeleton_series_run_empty, skeleton_src,
    };
    use crate::helper::test_helper::{contains_either, create_out_file};
    use rstest::rstest;
    use tempfile::TempDir;

    #[test]
    fn seriesreader_iter() {
        // test iterating without error
        let tmp_run = setup_series_dir();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&runs_dir).unwrap();
        assert_eq!(series_reader.run_count(), 3);

        // iterate over runs and observations
        for run in series_reader.iter() {
            for obs in run.iter() {
                assert!(obs.get("number").is_some());
                assert!(obs.get("word").is_some());
            }
        }
    }

    #[test]
    fn seriesreader_keys() {
        let tmp_run = setup_series_dir();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&runs_dir).unwrap();
        let keys = series_reader.keys();

        assert!(keys.contains(&"number"));
        assert!(keys.contains(&"word"));
        assert!(keys.len() == 2);
    }

    #[test]
    fn seriesreader_keys_no_content() {
        let tmp_run = setup_series_empty_out();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&runs_dir).unwrap();

        let keys = series_reader.keys();
        assert!(keys.contains(&"empty"));
        assert!(keys.len() == 1);

        let content = series_reader.get_runs()[0].get_var(keys[0]);
        assert!(content.is_some());
        assert_eq!(content.unwrap(), &vec![String::from("")]);
    }

    #[test]
    fn seriesreader_keys_no_out_files() {
        let tmp_run = setup_series_no_out();
        let series_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&series_dir).unwrap();
        let keys = series_reader.keys();

        assert!(keys.is_empty());
        assert!(series_reader.runs_are_empty());
    }

    #[rstest]
    fn seriesreader_serialize_multiline(
        #[from(skeleton_src)] outdir: TempDir,
        envlist_mixed_weird: OutList,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("2.csv");

        // not created yet
        assert!(!out_file.is_file());

        let reader = SeriesReader::from_out_lists(vec![envlist_mixed_weird]);
        reader.to_csv(&out_file).unwrap();

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
    fn seriesreader_serialize_single(
        #[from(skeleton_src)] outdir: TempDir,
        #[case] envlist: OutList,
        #[case] expected: String,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("0.csv");

        // not created yet
        assert!(!out_file.is_file());

        let reader = SeriesReader::from_out_lists(vec![envlist]);
        reader.to_csv(&out_file).unwrap();

        assert_eq!(std::fs::read_to_string(out_file).unwrap(), expected);
    }

    #[rstest]
    fn seriesreader_parse_empty(#[from(skeleton_src)] series_dir: TempDir) {
        let series_dir = series_dir.path().to_path_buf();
        let reader = SeriesReader::parse(&series_dir).unwrap();

        assert_eq!(reader.run_count(), 0);
        assert!(reader.runs_are_empty());
        assert!(reader.keys().is_empty());
    }

    #[rstest]
    fn seriesreader_parse_no_out(#[from(skeleton_series_run_empty)] series_dir: TempDir) {
        let series_dir = series_dir.path().to_path_buf();
        let reader = SeriesReader::parse(&series_dir).unwrap();

        assert_eq!(reader.run_count(), 1);
        assert!(reader.runs_are_empty());
        assert!(reader.keys().is_empty());
    }

    #[rstest]
    fn seriesreader_parse_empty_out(skeleton_series_run: TempDir) {
        let series_dir = skeleton_series_run.path().to_path_buf();
        let reader = SeriesReader::parse(&series_dir).unwrap();

        // key "empty" should be present, but without values
        assert_eq!(reader.run_count(), 1);
        let res = &reader.get_runs()[0];

        assert!(res.get_var("empty") == Some(&vec![String::new()]));
    }

    #[rstest]
    fn seriesreader_parse_no_value(filled_series_run_na: TempDir) {
        let series_dir = filled_series_run_na.path().to_path_buf();

        // both runs recognized
        let reader = SeriesReader::parse(&series_dir).unwrap();
        assert_eq!(reader.run_count(), 2);

        let mut series_iter = reader.get_runs().iter();
        let res = series_iter.next().unwrap();
        println!("res: {res:?}");
        assert_eq!(res.get_var("empty").unwrap(), &vec![String::from("NA")]); // "NA" from run_rep_dir_1

        let res = series_iter.next().unwrap();
        assert_eq!(res.get_var("empty").unwrap(), &vec![String::new()]); // empty string from run_rep_dir_0

        assert!(series_iter.next().is_none());
    }

    #[rstest]
    fn seriesreader_parse_duplicates(filled_series_run_duplicate: TempDir) {
        let series_dir = filled_series_run_duplicate.path().to_path_buf();
        let reader = SeriesReader::parse(&series_dir).unwrap();
        assert_eq!(reader.run_count(), 1);

        let res = &reader.get_runs()[0];

        assert!(res.get_var("some").is_some());
        assert!(res.get_var("some.txt").is_some());
    }

    #[rstest]
    fn seriesreader_parse_out_no_name(filled_series_run_invalid: TempDir) {
        let series_dir = filled_series_run_invalid.path().to_path_buf();
        assert!(SeriesReader::parse(&series_dir).is_err());
    }

    #[rstest]
    fn seriesreader_parse_multiline(skeleton_series_run: TempDir) {
        // add out files
        let series_dir = skeleton_series_run.path().to_path_buf();
        create_out_file(&series_dir, None, "out_single", "foo");
        create_out_file(&series_dir, None, "out_multi", "11\n20");
        create_out_file(&series_dir, None, "out_trailing", "11\n20");

        let reader = SeriesReader::parse(&series_dir).unwrap();
        assert_eq!(reader.run_count(), 1);
        let res = &reader.get_runs()[0];

        // check content, order is important
        assert!(res.get_var("multi").is_some());
        assert_eq!(
            res.get_var("multi").unwrap(),
            &vec!["11".to_string(), "20".to_string()]
        );

        // same as multi
        assert!(res.get_var("trailing").is_some());
        assert_eq!(
            res.get_var("trailing").unwrap(),
            &vec!["11".to_string(), "20".to_string()]
        );

        assert!(res.get_var("single").is_some());
        assert_eq!(
            res.get_var("single").unwrap(),
            &vec!["foo".to_string(), "foo".to_string()]
        );
    }

    #[rstest]
    fn seriesreader_parse_multiline_empty(skeleton_series_run: TempDir) {
        // add out files
        let series_dir = skeleton_series_run.path().to_path_buf();
        create_out_file(&series_dir, None, "out_multi", "foo\nbar");
        create_out_file(&series_dir, None, "out_empty", "");

        let reader = SeriesReader::parse(&series_dir).unwrap();
        assert_eq!(reader.run_count(), 1);
        let res = &reader.get_runs()[0];

        // check content
        assert!(res.get_var("multi").is_some());
        assert_eq!(
            res.get_var("multi").unwrap(),
            &vec!["foo".to_string(), "bar".to_string()]
        );

        assert!(res.get_var("empty").is_some());
        assert_eq!(
            res.get_var("empty").unwrap(),
            &vec![String::new(), String::new()]
        );
    }

    // If there are two values in the same run,
    // they have to have the same number of rows.
    #[rstest]
    fn seriesreader_parse_multiline_mismatch(skeleton_series_run: TempDir) {
        // add out files in both run reps
        let series_dir = skeleton_series_run.path().to_path_buf();
        create_out_file(&series_dir, None, "out_foo", "11\n20"); // two lines
        create_out_file(&series_dir, None, "out_bar", "6\n48\n15"); // three lines

        assert!(SeriesReader::parse(&series_dir).is_err());
    }

    // If there are multiple runs, then the number of rows in a value
    // can differ between
    #[rstest]
    fn seriesreader_parse_multiline_multiple_dirs_diff_length(filled_series_run_na: TempDir) {
        // add out files in both run reps
        let series_dir = filled_series_run_na.path().to_path_buf();
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_foo", "11\n20"); // two lines
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR1), "out_foo", "6\n48\n15"); // three lines

        let reader = SeriesReader::parse(&series_dir).unwrap();

        // check content
        assert!(!reader.runs_are_empty());
    }

    #[rstest]
    fn seriesreader_parse_output_full(filled_series_run_na: TempDir) {
        // add multiple out_ files and some that will not be used
        let series_dir = filled_series_run_na.path().to_path_buf();
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "not_out_file", "");
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "random", "");

        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_empty.txt", "");
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_some", "foo");
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR1), "out_some", "bar");

        // both runs parsed
        let reader = SeriesReader::parse(&series_dir).unwrap();
        assert_eq!(reader.run_count(), 2);
        let mut series_iter = reader.get_runs().iter();

        let res = series_iter.next().unwrap();
        println!("res: {res:?}");
        assert!(res.get_var("some").unwrap().contains(&String::from("bar")));
        assert!(res.get_var("empty").unwrap().contains(&String::from("NA")));
        assert!(res
            .get_var("empty.txt")
            .unwrap()
            .contains(&String::from("NA")));

        let res = series_iter.next().unwrap();
        println!("res: {res:?}");
        assert!(res.get_var("empty").unwrap().contains(&String::new()));
        assert!(res.get_var("empty.txt").unwrap().contains(&String::new()));
        assert!(res.get_var("some").unwrap().contains(&String::from("foo")));
    }

    #[rstest]
    fn seriesreader_invalid_trial(filled_series_run_na: TempDir) {
        let series_dir = filled_series_run_na.path().to_path_buf();
        let reader = SeriesReader::parse(&series_dir).unwrap();

        assert!(!reader.is_valid_trial());
    }

    #[rstest]
    fn seriesreader_valid_trial(setup_series_empty_out: TempDir) {
        let series_dir = setup_series_empty_out.path().to_path_buf();
        assert!(series_dir.is_dir());

        let reader = SeriesReader::parse(&series_dir).unwrap();

        assert!(reader.is_valid_trial());
    }
}
