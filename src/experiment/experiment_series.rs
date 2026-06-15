use crate::duplicate_log_to_file;
use crate::experiment::{
    experiment_run::ExperimentRun,
    experiment_source::ExperimentSource,
    out_file::{OutFile, OutList},
    CsvWriter, FileReader, FileWriter,
};
use crate::harness::env::Environment;
use crate::helper::{
    archivist::{copy_harness_dir, create_harness_dir, create_harness_file},
    errors::{Error, Result},
    fs_names::*,
};

use chrono::Local;
use csv::Writer;
use log::{debug, info, trace};
use rand::seq::SliceRandom;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

/// Container for an Experiment Series
#[derive(Debug, Clone)]
pub struct ExperimentSeries {
    source: ExperimentSource,
    path: Option<PathBuf>,
    runs: Vec<ExperimentRun>,
    stdout_log: Option<String>,
    stderr_log: Option<String>,
    exomat_log: Option<String>,
}

impl ExperimentSeries {
    pub fn from_source(source: &ExperimentSource) -> Self {
        Self {
            source: source.clone(),
            path: None,
            runs: Vec::new(),
            stdout_log: None,
            stderr_log: None,
            exomat_log: None,
        }
    }

    /// Immutable iteration
    pub fn iter<'a>(&'a self) -> SeriesReaderIter<'a> {
        SeriesReaderIter {
            series_reader: self,
            index: 0,
        }
    }

    pub fn repetition_count(&self) -> u64 {
        self.source.repetitions() * self.source.get_envs().environment_count()
    }

    pub fn experiment_name(&self) -> String {
        self.source.name()
    }

    pub fn exomat_envs(&self) -> Environment {
        self.source.exomat_envs()
    }

    pub fn run_script(&self) -> &str {
        self.source.run_script()
    }

    /// Returns the list of Experiment Runs.
    pub fn get_runs(&self) -> &Vec<ExperimentRun> {
        &self.runs
    }

    /// Returns a list of all keys present in the recorded RunReader in an arbitrary order.
    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .runs
            .iter()
            .filter_map(|run| run.get_out_files().as_ref())
            .flat_map(|outlist| outlist.iter().map(|outfile| outfile.var_name().as_str()))
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
    /// [Foo] out_uptime.s: "1234"
    /// [Foo] out_hello: "world"
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
                for out_file in outfiles.to_vec() {
                    eval_str.push_str(&format!("[{exp_name}] {out_file}\n"));
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

    pub fn generate_runs(&mut self) -> Result<()> {
        for environment in self.shuffled_environments() {
            let run = ExperimentRun::new(self.source.run_script(), &environment);
            trace!("Created run: {:?}", run);

            self.runs.push(run);
        }

        Ok(())
    }

    /// Build the filepath to a new series directory.
    ///
    /// Generates either a trial run location, or a new name in the PWD.
    ///
    /// The name will be derived from the experiment name and the current date and time.
    pub fn generate_series_filepath(exp_source: &Path) -> Result<PathBuf> {
        let format = format!("{}-%Y-%m-%d-%H-%M-%S", file_name_string(exp_source));
        let dirname = PathBuf::from(Local::now().format(&format).to_string());
        Ok(std::env::current_dir()?
            .canonicalize()?
            .join(&dirname)
            .to_path_buf())
    }

    /// Compiles a list of all repetition for each environment, then suffles said list.
    ///
    /// The shuffled list is then sorted by repetition, so that all n-repetitions run
    /// before all n+1-repetitions.
    fn shuffled_environments(&self) -> Vec<Environment> {
        let mut running_order = vec![];

        for env in self.source.get_envs().to_env_list() {
            for rep in 0..self.repetition_count() {
                // include the repetition in a tuple, so that it can be sorted correctly later
                running_order.push((env, rep));
            }
        }

        running_order.shuffle(&mut rand::rng());
        running_order.sort_by(|a, b| (a.1).cmp(&b.1));

        running_order.iter().map(|item| item.0.to_owned()).collect()
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
                        None => OutList::default(),
                        Some(r) => r.clone(),
                    };
                    new_run.push(OutFile::from(key, vec!["NA".to_string()]));

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
        let mut rows = OutList::default();
        for run in &self.runs {
            if let Some(out) = &run.get_out_files() {
                rows.extend_list(out)
            } else {
                rows.extend(Vec::new())
            }
        }

        // collect all header
        let mut rows_vec: Vec<Vec<String>> =
            vec![self.keys().iter().map(|k| k.to_string()).collect()];

        // turn all data into one list
        let val_len = rows.iter().map(|out| out.value_count()).max().unwrap_or(0);

        for i in 0..val_len {
            // (one entry = every ith element of each key)
            let mut row: Vec<String> = Vec::new();

            for key in self.keys() {
                let outfile = rows
                    .iter()
                    .find(|outfile| outfile.var_name() == key)
                    .expect(&format!("No outfile with name \"{}\" found", key));
                row.push(
                    outfile
                        .values()
                        .get(i)
                        .cloned()
                        .unwrap_or_else(|| String::new()),
                );
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

    /// helper, that returns the content of a file if it is readable.
    /// Otherwise returns `None`
    fn read_log(path: &PathBuf) -> Option<String> {
        match read_to_string(path) {
            Ok(log) => Some(log),
            Err(_) => None,
        }
    }

    /// Parses a SeriesReader from multiple OutLists (Test helper)
    ///
    /// One OutList represents the out_files of one RunReader.
    #[cfg(test)]
    fn from_out_lists(list_of_envlist: Vec<OutList>) -> Self {
        let runs: Vec<ExperimentRun> = list_of_envlist
            .iter()
            .map(|envlist| ExperimentRun::from_out_list_unchecked(&envlist))
            .collect();

        ExperimentSeries {
            source: ExperimentSource::new(),
            path: None,
            runs: runs,
            stdout_log: None,
            stderr_log: None,
            exomat_log: None,
        }
    }
}

// ========================== Writer ==========================
impl FileWriter for ExperimentSeries {
    /// Creates and populates a new experiment series directory.
    ///
    /// The new directory will have this structure:
    /// ```notest
    /// SERIES_DIR
    ///   |-> .exomat_series
    ///   |-> .src/
    ///   | |-> .exomat_source_cp  [replaces .exomat_source]
    ///   | \-> [copy of experiment source directory, read-only]
    ///   \-> runs/
    ///     |-> stdout.log [EMPTY]
    ///     |-> stderr.log [EMPTY]
    ///     \-> exomat.log [EMPTY]
    /// ```
    /// > Note: This example assumes values for [SERIES_SRC_DIR], [SERIES_RUNS_DIR], [SERIES_STDERR_LOG],
    /// > [SERIES_STDOUT_LOG], [SERIES_EXOMAT_LOG]. The names in the actual file structure might
    /// > differ, depending on the values of them.
    ///
    /// This function will not overwrite an existing series directory.
    ///
    /// Once the log files have been created any log output by exomat will be duplicated
    /// to them.
    ///
    /// ## Errors and Panics
    /// - Returns a `HarnessCreateError` if there is an experiment series directory
    ///   called `series_name` in the same directory
    /// - Panics if `exp_source` could not be read
    fn persist(&mut self, dir: &PathBuf) -> Result<()> {
        debug!(
            "attempting to build series directory from {}",
            self.source.location().display()
        );

        debug!("checking if is dir");
        if !self.source.location().is_dir() {
            return Err(Error::HarnessRunError {
                experiment: self.source.location().display().to_string(),
                err: "is not directory".to_string(),
            });
        }

        debug!("checking if source dir marker exists");
        if !self.source.location().join(MARKER_SRC).is_file() {
            return Err(Error::HarnessRunError {
                experiment: self.source.location().display().to_string(),
                err: "is not an experiment source directory".to_string(),
            });
        }

        // check if series dir is valid
        fn is_child_dir_of_of(maybe_child: &Path, parent: &Path) -> Result<bool> {
            let parent = parent.canonicalize()?;

            Ok(maybe_child
                .ancestors()
                .any(|ancestor| match ancestor.canonicalize() {
                    Ok(ancestor) => ancestor == parent,
                    Err(_) => false, // dir does not exist -> is certainly not parent
                }))
        }

        debug!("checking if creating series inside of experiment (would be forbidden)");
        if is_child_dir_of_of(dir, self.source.location())? {
            // log full paths to debug, but let error be handled (i.e. reported as error) outside
            debug!("refusing to build series dir inside of experiment dir, experiment dir: {}, to-be-created series dir: {}",
               self.source.location().display(),
               dir.display());
            return Err(Error::HarnessRunError {
                experiment: self.source.location().display().to_string(),
                err: "can not generate output inside of experiment dir".to_string(),
            });
        }

        let src = create_harness_dir(&dir.join(SERIES_SRC_DIR))?;
        let runs = create_harness_dir(&dir.join(SERIES_RUNS_DIR))?;

        let _ = create_harness_file(&dir.join(MARKER_SERIES))?;
        let _ = create_harness_file(&runs.join(SERIES_STDOUT_LOG))?;
        let _ = create_harness_file(&runs.join(SERIES_STDERR_LOG))?;
        let exomat_log = create_harness_file(&runs.join(SERIES_EXOMAT_LOG))?;

        duplicate_log_to_file(&exomat_log);

        // copy exp_source/template to src and replace marker
        copy_harness_dir(self.source.location(), &src)?;
        std::fs::remove_file(src.join(MARKER_SRC))?;
        create_harness_file(&src.join(MARKER_SRC_CP))?;

        info!("Created new experiment series dir at {}", dir.display());
        self.path = Some(dir.to_path_buf());

        Ok(())
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentSeries {
    type Item = ExperimentSeries;

    /// Parses an Experiment Series into a SeriesReader object.
    ///
    /// If `out_$NAME` is found in one experiment run directory, but not in another, a "NA"
    /// will be added to the list of values.
    ///
    /// ### Error
    /// - Returns a `ReaderError` if any RunReader failed to parse
    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        // find all run dirs
        let runs: Vec<ExperimentRun> = find_run_repetitions(&dir.join(SERIES_RUNS_DIR))
            .iter()
            .map(|run| {
                ExperimentRun::parse(run).map_err(|e| Error::ReaderError {
                    dir: run.display().to_string(),
                    reason: e.to_string(),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // read log files
        let stdout_log = Self::read_log(&dir.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG));
        let stderr_log = Self::read_log(&dir.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG));
        let exomat_log = Self::read_log(&dir.join(SERIES_RUNS_DIR).join(SERIES_EXOMAT_LOG));

        let mut reader = ExperimentSeries {
            source: ExperimentSource::new(),
            path: Some(dir.to_path_buf()),
            runs,
            stdout_log,
            stderr_log,
            exomat_log,
        };

        reader.fill_missing_keys();
        Ok(reader)
    }
}

// ========================== Writer ==========================
impl CsvWriter for ExperimentSeries {
    /// Serializes it's content into `file`.
    ///
    /// If the no runs are found or all runs are empty, `file` will still be created.
    ///
    /// Uses the default CSV delimiter `,`. Any values containing it will be escaped using
    /// `""`.
    ///
    /// ## Errors
    /// - Returns a `CsvError` if something went wrong during the csv serialization
    fn to_csv(&self, file: &PathBuf) -> Result<()> {
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
}

// ========================== Iterator ==========================
pub struct SeriesReaderIter<'a> {
    series_reader: &'a ExperimentSeries,
    index: usize,
}

impl<'a> Iterator for SeriesReaderIter<'a> {
    type Item = ExperimentRun;

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
impl<'a> IntoIterator for &'a ExperimentSeries {
    type Item = ExperimentRun;
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
        filled_series_run_duplicate, filled_series_run_invalid, filled_series_run_na, outlist_1a,
        outlist_empty_string, outlist_mixed_weird, outlist_one_var_no_val, setup_series_dir,
        setup_series_empty_out, setup_series_no_out, skeleton_series_run,
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

        let series_reader = ExperimentSeries::parse(&runs_dir).unwrap();
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

        let series_reader = ExperimentSeries::parse(&runs_dir).unwrap();
        let keys = series_reader.keys();

        assert!(keys.contains(&"number"));
        assert!(keys.contains(&"word"));
        assert!(keys.len() == 2);
    }

    #[test]
    fn seriesreader_keys_no_content() {
        let tmp_run = setup_series_empty_out();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = ExperimentSeries::parse(&runs_dir).unwrap();

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

        let series_reader = ExperimentSeries::parse(&series_dir).unwrap();
        let keys = series_reader.keys();

        assert!(keys.is_empty());
        assert!(series_reader.runs_are_empty());
    }

    #[rstest]
    fn seriesreader_serialize_multiline(
        #[from(skeleton_src)] outdir: TempDir,
        outlist_mixed_weird: OutList,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("2.csv");

        // not created yet
        assert!(!out_file.is_file());

        let reader = ExperimentSeries::from_out_lists(vec![outlist_mixed_weird]);
        reader.to_csv(&out_file).unwrap();

        // with multiple keys and values the order of items after serialization is
        // random, so only check if the correct lines are there
        let file_2 = std::fs::read_to_string(out_file).unwrap();
        assert!(contains_either(&file_2, "VAR1,VAR2\n", "VAR2,VAR1\n"));
        assert!(contains_either(&file_2, "VALUE,\n", ",VALUE\n"));
        assert!(contains_either(&file_2, "\"a,b\",baz\n", "baz,\"a,b\"\n"));
    }

    #[rstest]
    #[case(OutList::default(), "")]
    #[case(outlist_1a(), "1\na\n")]
    #[case(outlist_one_var_no_val(), "VAR\n")]
    #[case(outlist_empty_string(), "VAR\n\"\"\n")]
    fn seriesreader_serialize_single(
        #[from(skeleton_src)] outdir: TempDir,
        #[case] outlist: OutList,
        #[case] expected: String,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("0.csv");

        // not created yet
        assert!(!out_file.is_file());

        let reader = ExperimentSeries::from_out_lists(vec![outlist]);
        reader.to_csv(&out_file).unwrap();

        assert_eq!(std::fs::read_to_string(out_file).unwrap(), expected);
    }

    #[rstest]
    fn seriesreader_parse_empty(#[from(skeleton_src)] series_dir: TempDir) {
        let series_dir = series_dir.path().to_path_buf();
        let reader = ExperimentSeries::parse(&series_dir).unwrap();

        assert_eq!(reader.run_count(), 0);
        assert!(reader.runs_are_empty());
        assert!(reader.keys().is_empty());
    }

    #[rstest]
    fn seriesreader_parse_no_out(#[from(skeleton_series_run_empty)] series_dir: TempDir) {
        let series_dir = series_dir.path().to_path_buf();
        let reader = ExperimentSeries::parse(&series_dir).unwrap();

        assert_eq!(reader.run_count(), 1);
        assert!(reader.runs_are_empty());
        assert!(reader.keys().is_empty());
    }

    #[rstest]
    fn seriesreader_parse_empty_out(skeleton_series_run: TempDir) {
        let series_dir = skeleton_series_run.path().to_path_buf();
        let reader = ExperimentSeries::parse(&series_dir).unwrap();

        // key "empty" should be present, but without values
        assert_eq!(reader.run_count(), 1);
        let res = &reader.get_runs()[0];

        assert!(res.get_var("empty") == Some(&vec![String::new()]));
    }

    #[rstest]
    fn seriesreader_parse_no_value(filled_series_run_na: TempDir) {
        let series_dir = filled_series_run_na.path().to_path_buf();

        // both runs recognized
        let reader = ExperimentSeries::parse(&series_dir).unwrap();
        let runs = reader.get_runs();

        let expected0 =
            OutList::from(vec![OutFile::from("empty", vec![String::from("")])]).unwrap();
        let expected1 =
            OutList::from(vec![OutFile::from("empty", vec![String::from("NA")])]).unwrap();

        assert_eq!(reader.run_count(), 2);
        assert!(runs.contains(&ExperimentRun::from_out_list_unchecked(&expected0)));
        assert!(runs.contains(&ExperimentRun::from_out_list_unchecked(&expected1)));
    }

    #[rstest]
    fn seriesreader_parse_duplicates(filled_series_run_duplicate: TempDir) {
        let series_dir = filled_series_run_duplicate.path().to_path_buf();
        let reader = ExperimentSeries::parse(&series_dir).unwrap();
        assert_eq!(reader.run_count(), 1);

        let res = &reader.get_runs()[0];

        assert!(res.get_var("some").is_some());
        assert!(res.get_var("some.txt").is_some());
    }

    #[rstest]
    fn seriesreader_parse_out_no_name(filled_series_run_invalid: TempDir) {
        let series_dir = filled_series_run_invalid.path().to_path_buf();
        assert!(ExperimentSeries::parse(&series_dir).is_err());
    }

    #[rstest]
    fn seriesreader_parse_multiline(skeleton_series_run: TempDir) {
        // add out files
        let series_dir = skeleton_series_run.path().to_path_buf();
        create_out_file(&series_dir, None, "out_single", "foo");
        create_out_file(&series_dir, None, "out_multi", "11\n20");
        create_out_file(&series_dir, None, "out_trailing", "11\n20");

        let reader = ExperimentSeries::parse(&series_dir).unwrap();
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

        let reader = ExperimentSeries::parse(&series_dir).unwrap();
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

        assert!(ExperimentSeries::parse(&series_dir).is_err());
    }

    // If there are multiple runs, then the number of rows in a value
    // can differ between
    #[rstest]
    fn seriesreader_parse_multiline_multiple_dirs_diff_length(filled_series_run_na: TempDir) {
        // add out files in both run reps
        let series_dir = filled_series_run_na.path().to_path_buf();
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR0), "out_foo", "11\n20"); // two lines
        create_out_file(&series_dir, Some(TEST_RUN_REP_DIR1), "out_foo", "6\n48\n15"); // three lines

        let reader = ExperimentSeries::parse(&series_dir).unwrap();

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
        let reader = ExperimentSeries::parse(&series_dir).unwrap();
        let runs = reader.get_runs();

        // check results
        let some0 = OutFile::from("some", vec![String::from("bar")]);
        let empty0 = OutFile::from("empty", vec![String::from("NA")]);
        let emptytxt0 = OutFile::from("empty.txt", vec![String::from("NA")]);

        let some1 = OutFile::from("some", vec![String::from("foo")]);
        let empty1 = OutFile::from("empty", vec![String::from("")]);
        let emptytxt1 = OutFile::from("empty.txt", vec![String::from("")]);

        assert_eq!(reader.run_count(), 2);

        // since the order of OutFiles per run is not always the same, test it this way
        for run in runs {
            let outlist = run.get_out_files().as_ref().unwrap();
            assert_eq!(outlist.len(), 3);

            assert!(
                outlist.contains(&some0)
                    && outlist.contains(&empty0)
                    && outlist.contains(&emptytxt0)
                    || outlist.contains(&some1)
                        && outlist.contains(&empty1)
                        && outlist.contains(&emptytxt1)
            )
        }
    }

    #[rstest]
    fn seriesreader_invalid_trial(filled_series_run_na: TempDir) {
        let series_dir = filled_series_run_na.path().to_path_buf();
        let reader = ExperimentSeries::parse(&series_dir).unwrap();

        assert!(!reader.is_valid_trial());
    }

    #[rstest]
    fn seriesreader_valid_trial(setup_series_empty_out: TempDir) {
        let series_dir = setup_series_empty_out.path().to_path_buf();
        assert!(series_dir.is_dir());

        let reader = ExperimentSeries::parse(&series_dir).unwrap();

        assert!(reader.is_valid_trial());
    }
}
