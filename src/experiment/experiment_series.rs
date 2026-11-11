use super::SeriesStatus;
use crate::experiment::{
    experiment_run::RunStatus, out_file::OutFile, CsvWriter, ExperimentRun, ExperimentSource,
    FileReader, FileWriter,
};
use crate::harness::env::{Environment, ExomatEnvironment};
use crate::helper::{
    archivist::{copy_harness_dir, create_harness_dir, create_harness_file},
    errors::{Error, Result},
    fs_names::*,
};

use chrono::Local;
use csv::Writer;
use log::{debug, error, info, trace};
use rand::seq::SliceRandom;
use std::path::{Path, PathBuf};

/// Container for an Experiment Series
///
/// An Experiment Series must be written to the filesystem before it can be exeucted.
/// A normal workflow would be:
/// 1. `from_source(...)`
/// 2. `generate_runs()`
/// 2. `persist(...)`
/// 3. `for run in series.runs.iter_mut() { run.execute(); }`
#[derive(Debug, PartialEq)]
pub struct ExperimentSeries {
    pub source: ExperimentSource,
    pub path: Option<PathBuf>,
    pub runs: Vec<ExperimentRun>,
}

impl TryFrom<&ExperimentSource> for ExperimentSeries {
    type Error = Error;

    /// Gernerate an Experiment Series based on source
    ///
    /// The ExperimentSeries will have the following values set:
    /// - `source`: copy of source
    /// - `path`: output of ExperimentSeries::generate_series_filepath()
    /// - `runs`: empty Vector
    ///
    /// ## Errors
    /// - retruns a `HarnessRunError` if source.location is PWD
    /// - returns an `IoError` if it cannot parse a valid Series name
    fn try_from(_source: &ExperimentSource) -> Result<Self> {
        todo!("remove");
    }
}

impl ExperimentSeries {
    /// generate a new series on disk
    ///
    /// Generate `repetitions` runs for each env in `source`.
    /// Errors if `series_dst` already exists.
    ///
    /// Appends path to `source` and repetion number to each iteration.
    pub fn create(
        source: &ExperimentSource,
        series_dst: PathBuf,
        repetitions: u32,
    ) -> Result<Self> {
        todo!();
    }

    /// Return a string describing the overall success of the Experiment Series
    ///
    /// - If any Experiment Run in self.runs failed, return `Failed. Reason: [...]`
    /// - If all Experiment Runs were successful, return `Successful`
    /// - If any Experiment Run has not been executed or its status in Unknown, return `Cannot determine run status`
    pub fn series_status(&self) -> Result<SeriesStatus> {
        todo!("move status into separate struct");
        let runs_total = self.runs.len();
        let runs_success = self
            .runs
            .iter()
            .filter(|run| matches!(run.status(), Ok(RunStatus::Success)))
            .count();
        let runs_not_started = self
            .runs
            .iter()
            .filter(|run| matches!(run.status(), Ok(RunStatus::NotStarted)))
            .count();
        let runs_failed = runs_total - runs_not_started - runs_success;

        let failure_desc = self
            .runs
            .iter()
            .filter_map(|run| {
                let run_name = run.run_name();
                let run_location = run
                    .location()
                    .display();
                match run.status() {
                    Err(err) => Some(format!("failed to read status of {run_name} at {run_location}: {err}")),
                    Ok(RunStatus::NotStarted) | Ok(RunStatus::Success) => None,
                    Ok(RunStatus::ExitNonZero(exit_code)) => Some(format!(
                            "run {run_name} at {run_location} exited non-zero, please review logs. exit code: {exit_code}")),
                }}
            )
            .fold(String::new(), |a, b| a + "\n" + b.as_str());

        format!("{runs_success}/{runs_total} successful, {runs_not_started}/{runs_not_started} not started, {runs_failed}/{runs_total} failed{failure_desc}")
    }

    /// Generate Experiment Runs based on the current Experiment Series
    ///
    /// Every defined Environment will be used `self.source.repetitions()` times.
    /// This means `self.source.repetitions() * self.envs.len()` Experiment Runs will be created.
    ///
    /// If no Environemnts are defined, an empty Environment will be used.
    /// May create no Experiment Runs, depending on the given repetition number.
    ///
    /// ## Errors
    /// - returns an `Empty` Error, if self.path is empty
    pub fn generate_runs(&mut self) -> Result<()> {
        todo!("refactor after experiment run refactor");
        // if self.path.is_none() {
        //     return Err(Error::Empty(String::from("Series location not set")));
        // }
        //
        // if *self.source.repetitions() > 1 {
        //     warn!("Repetition set to less than 1. No Experiment Runs will be created.");
        // }
        //
        // // helper
        // fn generate_run_from(
        //     series: &ExperimentSeries,
        //     env: (&PathBuf, &Environment),
        //     repetition: u64,
        // ) -> ExperimentRun {
        //     let exomat_envs = ExomatEnvironment::new(series.source.location(), repetition);
        //
        //     ExperimentRun::new(
        //         series.source.run_script(),
        //         env,
        //         &exomat_envs,
        //         series.source.repetitions().to_string().len(),
        //     )
        // }
        //
        // let mut run_list = Vec::new();
        //
        // if self.source.envs().is_empty() {
        //     for rep in 0..*self.source.repetitions() {
        //         // cannot edit self.runs directly here, beucase of the borrow checker :)
        //         run_list.push(generate_run_from(
        //             self,
        //             (&PathBuf::from(SRC_ENV_FILE), &Environment::new()),
        //             rep,
        //         ));
        //     }
        // } else {
        //     for (environment, rep) in self.shuffled_environments() {
        //         run_list.push(generate_run_from(self, environment, rep));
        //     }
        // }
        //
        // self.runs.extend(run_list);
        // Ok(())
    }

    /// Build the filepath to a new series directory.
    ///
    /// The name will be derived from the experiment name and the current date and time.
    ///
    /// ## Errors
    /// - returns an `IoError` if the current directory is inaccessable
    pub fn generate_series_filepath(exp_source: &Path) -> Result<PathBuf> {
        todo!("move into binary");
        let format = format!("{}-%Y-%m-%d-%H-%M-%S", file_name_string(exp_source));
        let dirname = PathBuf::from(Local::now().format(&format).to_string());
        Ok(std::env::current_dir()?
            .canonicalize()?
            .join(&dirname)
            .to_path_buf())
    }

    /// Returns the Experiment name, taken from the Experiment Source of this Experiment Series
    ///
    /// For errors, see `ExperimentSource::name()`
    pub fn series_name(&self) -> Result<String> {
        todo!("basename of dir");
    }

    /// Returns the location in the filesystem of this Experiment Series
    ///
    /// Is `None` if the Series has not been serialized, this
    pub fn location(&self) -> &PathBuf {
        todo!();
    }

    /// Returns the list of Experiment Runs.
    pub fn runs(&self) -> &Vec<ExperimentRun> {
        todo!();
    }

    /// execute all runs
    ///
    /// - `failfast`:
    ///    if set to true, abort on first failed run execution.
    ///    if set to false keep executing after failure
    /// - `resume`: resume existing series.
    ///    if set to true, try to run all runs with status not started.
    ///    if set to false, abort if there is a single run that is already started.
    pub fn execute(&self, failfast: bool, resume: bool) -> Result<()> {
        todo!("replace w/ unified execute");
    }

    /// execute all runs; abort if any have been started already
    pub fn execute_all(&self, failfast: bool) -> Result<()> {
        todo!("replace w/ unified execute");
    }

    /// execute all runs which have not been started yet
    ///
    /// if failfast is enabled: abort on first failure, otherwise continue on error
    ///
    /// Note: if there is an error in the infrastructure, the execution is aborted instantaneously
    pub fn execute_not_started(&self, failfast: bool) -> Result<()> {
        todo!("replace w/ unified execute");
    }

    // =========== TODO remove below ========================

    /// retrieve all names of outfiles, i.e. XXX for `out_XXX`
    pub fn output_keys(&self) -> Vec<String> {
        todo!("remove");
        // let mut keys: Vec<String> = self
        //     .runs
        //     .iter()
        //     .flat_map(|run| {
        //         run.out_files()
        //             .unwrap()
        //             .iter()
        //             .map(|outfile| outfile.var_name().to_string())
        //             .collect::<Vec<String>>()
        //     })
        //     .collect();
        //
        // // remove duplicate keys
        // keys.sort();
        // keys.dedup();
        // keys
    }

    /// retrieve all names of variables in all environments
    pub fn env_keys(&self) -> Result<Vec<String>> {
        todo!("remove");
        // let mut keys: Vec<String> = self
        //     .runs
        //     .iter()
        //     .map(|run| {
        //         Ok(run
        //             .env()?
        //             .get_env_vars()
        //             .iter()
        //             .map(|s| s.to_string())
        //             .collect::<Vec<String>>())
        //     })
        //     .collect::<Result<Vec<Vec<String>>>>()?
        //     .into_iter()
        //     .flatten()
        //     .collect();
        //
        // keys.sort();
        // keys.dedup();
        // Ok(keys)
    }

    // TODO remove
    // /// Adds missing out_ files to each Experiment Run.
    // ///
    // /// If a key is present in one Experiment Run but missing another, the key will be
    // /// added with "NA" as it's value.
    // fn fill_missing_keys(&mut self) {
    //     let keys: Vec<String> = self
    //         .output_keys()
    //         .into_iter()
    //         .map(|k| k.to_string())
    //         .collect();
    //
    //     for run in self.runs.iter_mut() {
    //         for key in &keys {
    //             if run.out_var(key).is_none() {
    //                 run.insert_out_file(OutFile::new(key, vec!["NA".to_string()]));
    //             }
    //         }
    //     }
    // }

    /// Parses `self.runs` into rows, that can be serialized in a CSV format.
    /// Includes a header row
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
        todo!("mv smwhere else/rm altogether");
        // sort runs by their repetition/env
        let mut sorted_runs = self.runs.clone();
        sorted_runs.sort_by_key(|run| run.run_name().to_owned());

        todo!("rm unwrap");
        // collect all header
        let header: Vec<String> = [self.env_keys().unwrap(), self.output_keys()]
            .concat()
            .iter()
            .map(|s| s.to_string())
            .collect();

        let mut rows_vec: Vec<Vec<String>> = vec![header];

        let max_val_len = sorted_runs
            .iter()
            .map(|run| {
                run.out_files()
                    .unwrap()
                    .iter()
                    .map(|out_file| out_file.values().len())
                    .max()
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0);

        todo!("refactor to not rely on key ordering");
        // collect content (one entry = every ith element of each key)
        // for i in 0..max_val_len {
        //     // for each run ...
        //     for run in &sorted_runs {
        //         let mut row: Vec<String> = Vec::new();
        //
        //         // add env
        //         for env_var in self.env_keys() {
        //             row.push(run.env().get_env_val(env_var).cloned().unwrap_or_default());
        //         }
        //
        //         // add output
        //         for key in self.output_keys() {
        //             if let Some(vals) = &run.out_var(key) {
        //                 row.push(vals.get(i).cloned().unwrap_or_else(String::new));
        //             } else {
        //                 row.push(String::new())
        //             }
        //         }
        //
        //         // ... and save the list for this run
        //         rows_vec.push(row);
        //     }
        // }

        rows_vec
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
    fn to_csv(&self, csv_file: &Path) -> Result<()> {
        todo!("rm");
        // let mut wtr = Writer::from_path(csv_file).map_err(|e| Error::CsvError {
        //     reason: e.to_string(),
        // })?;
        //
        // if !self.runs_are_empty() {
        //     // turn self.runs into csv rows (contains header)
        //     let content = self.to_csv_rows();
        //     debug!("series contains content: {:?}", content);
        //
        //     for row in content {
        //         wtr.write_record(row).map_err(|e| Error::CsvError {
        //             reason: e.to_string(),
        //         })?;
        //     }
        // }
        //
        // wtr.flush().map_err(|e| Error::CsvError {
        //     reason: e.to_string(),
        // })
    }
}

impl FileWriter for ExperimentSeries {
    /// Serializes the Experiment Series into a directory.
    ///
    /// The new directory will have this structure:
    /// ```notest
    /// SERIES_DIR
    ///   |-> .exomat_series
    ///   |-> [SERIES_SRC_DIR]
    ///   | |-> .exomat_source_cp  [replaces .exomat_source]
    ///   | \-> [copy of experiment source directory, read-only]
    ///   \-> [SERIES_RUNS_DIR]
    ///     | |-> [run rep dir 1]
    ///     | | \-> [see ExperimentRun::persist()]
    ///     | \-> [run rep dir n...]
    ///     \-> [SERIES_EXOMAT_LOG]
    /// ```
    /// This function will not overwrite an existing series directory.
    ///
    /// Once the exomat log has been created, any output by exomat will be duplicated
    /// to them.
    ///
    /// ## Errors and Panics
    /// - Returns a `HarnessCreateError` if there is an experiment series directory
    ///   called `series_name` in the same directory
    /// - Panics if `exp_source` could not be read
    fn persist(&mut self, exp_series_dir: &Path) -> Result<()> {
        todo!("refactor series generator");
        // debug!(
        //     "attempting to build series directory from {}",
        //     self.source.location().display()
        // );
        //
        // trace!("checking if is dir");
        // if !self.source.location().is_dir() {
        //     return Err(Error::HarnessRunError {
        //         experiment: self.source.location().display().to_string(),
        //         err: "is not directory".to_string(),
        //     });
        // }
        //
        // trace!("checking if source dir marker exists");
        // if !self.source.location().join(MARKER_SRC).is_file() {
        //     return Err(Error::HarnessRunError {
        //         experiment: self.source.location().display().to_string(),
        //         err: "is not an experiment source directory".to_string(),
        //     });
        // }
        //
        // // check if series dir is valid
        // fn is_child_dir_of_of(maybe_child: &Path, parent: &Path) -> Result<bool> {
        //     let parent = parent.canonicalize()?;
        //
        //     Ok(maybe_child
        //         .ancestors()
        //         .any(|ancestor| match ancestor.canonicalize() {
        //             Ok(ancestor) => ancestor == parent,
        //             Err(_) => false, // dir does not exist -> is certainly not parent
        //         }))
        // }
        //
        // debug!("checking if creating series inside of experiment (would be forbidden)");
        // if is_child_dir_of_of(exp_series_dir, self.source.location())? {
        //     // log full paths to debug, but let error be handled (i.e. reported as error) outside
        //     error!("refusing to build series dir inside of experiment dir, experiment dir: {}, to-be-created series dir: {}",
        //        self.source.location().display(),
        //        exp_series_dir.display());
        //     return Err(Error::HarnessRunError {
        //         experiment: self.source.location().display().to_string(),
        //         err: "can not generate output inside of experiment dir".to_string(),
        //     });
        // }
        // let src = create_harness_dir(&exp_series_dir.join(SERIES_SRC_DIR))?;
        // let runs = create_harness_dir(&exp_series_dir.join(SERIES_RUNS_DIR))?;
        //
        // create_harness_file(&exp_series_dir.join(MARKER_SERIES))?;
        //
        // let logfile = runs.join(SERIES_EXOMAT_LOG);
        // create_harness_file(&logfile)?;
        // crate::helper::logging::set_logfile(&logfile)?;
        //
        // // copy exp_source/template to src and replace marker
        // copy_harness_dir(self.source.location(), &src)?;
        // std::fs::remove_file(src.join(MARKER_SRC))?;
        // create_harness_file(&src.join(MARKER_SRC_CP))?;
        //
        // // create runs if there are any to be created
        // for run in &mut self.runs {
        //     run.persist(&runs.join(run.run_dir_name()))?;
        // }
        //
        // info!(
        //     "Created new experiment series dir at {}",
        //     exp_series_dir.display()
        // );
        // self.path = Some(exp_series_dir.to_path_buf());
        //
        // Ok(())
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentSeries {
    type Item = ExperimentSeries;

    /// Parses an Experiment Series directory into an ExperimentSeries object.
    ///
    /// ### Error
    /// - Returns a `ReaderError` if any RunReader failed to parse
    fn parse(exp_series_dir: &Path) -> Result<Self::Item> {
        debug!("looking for experiment runs");
        let runs =
            <ExperimentSeries as FileReader>::find_all_files(&exp_series_dir.join(SERIES_RUNS_DIR))
                .iter()
                .map(|run| {
                    ExperimentRun::parse(run).map_err(|e| Error::ReaderError {
                        dir: run.display().to_string(),
                        reason: e.to_string(),
                    })
                })
                .collect::<Result<Vec<_>>>()?;

        let mut reader = ExperimentSeries {
            source: todo!(),
            path: Some(exp_series_dir.to_path_buf()),
            runs,
        };

        debug!("adding missing keys");
        // reader.fill_missing_keys();
        todo!("fill missing keys");
        Ok(reader)
    }

    /// Builds and returns a vector of all run repetitions in the given directory.
    ///
    /// A directory is considered a run repetition, if it's name starts with "run_".
    ///
    /// ## Panics
    /// - Panics if directory traversal went wrong
    fn find_all_files(runs_dir: &Path) -> Vec<PathBuf> {
        let mut repetitions = Vec::<PathBuf>::new();

        // return the empty vector if runs_dir does not exist
        if !runs_dir.is_dir() {
            trace!("no experiment runs found");
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
                    .expect("unreadyble entry")
                    .path() // complete path
                    .file_name() // last part of path; directory name
                    .expect("entry has inaccessable file name")
                    .to_str()
                    .expect("cannot stringify file name")
                    .starts_with("run_")
                {
                    trace!("found run: {}", entry.as_ref().unwrap().path().display());
                    repetitions.push(entry.unwrap().path());
                }
            }
        }

        repetitions
    }
}

// ========================== Display ==========================
impl std::fmt::Display for ExperimentSeries {
    /// Prints a report of the Experiment output in this Experiment Series
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let exp_name = self.source.name().map_err(|_| std::fmt::Error)?;

        let outfiles: String = todo!("mention available out_ files");
        let path = match &self.path {
            Some(path) => path
                .to_str()
                .expect("path contains non-UTF8 characters, wtf?!")
                .to_string(),
            None => "no path set (wtf?!)".to_string(),
        };

        // note: status is not read from disk as of now
        // mitigation: skip status
        // see #57
        // status: {status}
        write!(
            f,
            r#"=*= Summary =*=
experiment name: {exp_name}
path: {path}
out_ files:
{outfiles}
"#,
            // status = self.series_status()
        )
    }
}

// ========================== Tests ==========================

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_fixtures::{env_a1, env_foo};
    use rstest::{fixture, rstest};
    use tempfile::TempDir;

    struct SeriesWithHandles {
        // handles to keep dir w/ run alive
        tempdir: TempDir,
        // run.sh succeeds
        source: ExperimentSource,
        // run.sh fails
        source_fails: ExperimentSource,
        series: ExperimentSeries,
        series_fails: ExperimentSeries,
    }

    #[fixture]
    fn series_with_handles(env_a1: Environment, env_foo: Environment) -> SeriesWithHandles {
        let populate_src = |dir: PathBuf, new_run_sh: Option<&str>| -> ExperimentSource {
            let source = ExperimentSource::create_skeleton(dir).unwrap();
            env_a1.to_file(&source.envs_dir().join("0.env")).unwrap();
            env_foo.to_file(&source.envs_dir().join("1.env")).unwrap();

            if let Some(new_run_sh) = new_run_sh {
                std::fs::write(source.template().join(SRC_RUN_FILE), new_run_sh).unwrap()
            }

            source
        };

        let tempdir = tempfile::TempDir::new().unwrap();
        let src_dst = tempdir.path().join("source_dir");
        let source = populate_src(src_dst, None);

        let src_fails_dst = tempdir.path().join("source_fails_dir");
        let source_fails = populate_src(
            src_fails_dst,
            Some(
                "#!/usr/bin/env bash
set -euo pipefail

false",
            ),
        );

        let series_dst = tempdir.path().join("series");
        let series = ExperimentSeries::create(&source, series_dst, 1).unwrap();

        let series_fails_dst = tempdir.path().join("series_fails");
        let series_fails = ExperimentSeries::create(&source, series_fails_dst, 5).unwrap();

        SeriesWithHandles {
            tempdir,
            source,
            source_fails,
            series,
            series_fails,
        }
    }

    #[rstest]
    fn create_simple(
        series_with_handles: SeriesWithHandles,
        env_a1: Environment,
        env_foo: Environment,
    ) {
        std::fs::write(
            series_with_handles.source.location().join("sentinel_file"),
            "",
        )
        .unwrap();

        let series_dst = series_with_handles.tempdir.path().join("new_series");
        let series =
            ExperimentSeries::create(&series_with_handles.source, series_dst.clone(), 1).unwrap();

        assert_eq!(*series.location(), series_dst);

        // ensure one run per env
        let all_run_envs = series
            .runs()
            .iter()
            .map(|run| run.env().unwrap())
            .collect::<Vec<_>>();
        assert!(all_run_envs.contains(&env_a1));
        assert!(all_run_envs.contains(&env_foo));
        assert_eq!(2, all_run_envs.len());

        // smoke-test that source dir is copied
        let sentinel_copy_path = series.location().join(SERIES_SRC_DIR).join("sentinel_file");
        assert!(sentinel_copy_path.is_file());
    }

    #[rstest]
    fn create_already_exists(series_with_handles: SeriesWithHandles) {
        let existing_dir = series_with_handles.tempdir.path().join("hdhjasd");
        std::fs::create_dir(&existing_dir).unwrap();

        assert!(ExperimentSeries::create(&series_with_handles.source, existing_dir, 1).is_err());
    }

    #[rstest]
    fn zero_repetitions(series_with_handles: SeriesWithHandles) {
        let series_dst = series_with_handles.tempdir.path().join("new_series");
        assert!(
            ExperimentSeries::create(&series_with_handles.source, series_dst.clone(), 0).is_err()
        );
    }

    #[rstest]
    fn create_repetitions(
        series_with_handles: SeriesWithHandles,
        #[values(1, 2, 3, 4)] repetitions: u32,
    ) {
        let series_dst = series_with_handles.tempdir.path().join("new_series");
        let series =
            ExperimentSeries::create(&series_with_handles.source, series_dst, repetitions).unwrap();

        assert_eq!(2 * repetitions as usize, series.runs().len());
    }

    #[rstest]
    fn repetition_vars_injected(
        series_with_handles: SeriesWithHandles,
        #[values(1, 2, 3, 4)] repetitions: u32,
    ) {
        let series_dst = series_with_handles.tempdir.path().join("new_series");
        let series =
            ExperimentSeries::create(&series_with_handles.source, series_dst, repetitions).unwrap();

        let mut all_repetition_env_vals = Vec::new();
        for run in series.runs() {
            let env = run.env().unwrap();
            let repetition_val = env.get_env_val("REPETITION").unwrap().to_string();
            all_repetition_env_vals.push(repetition_val);
        }

        let mut expected_repetition_vals = (1..=repetitions)
            .chain(1..=repetitions)
            .map(|x| x.to_string())
            .collect::<Vec<String>>();

        expected_repetition_vals.sort();
        all_repetition_env_vals.sort();

        assert_eq!(expected_repetition_vals, all_repetition_env_vals);
    }

    #[rstest]
    fn smoke_read_status_from_disk(series_with_handles: SeriesWithHandles) {
        let series = &series_with_handles.series;
        let series_status = series.series_status().unwrap();
        assert_eq!(series.runs().len(), series_status.total());
        assert_eq!(series.runs().len(), series_status.not_started);
    }

    #[rstest]
    fn parse_from_disk(series_with_handles: SeriesWithHandles) {
        let loaded_series =
            ExperimentSeries::parse(series_with_handles.series.location().as_path()).unwrap();
        assert_eq!(loaded_series, series_with_handles.series);
    }

    #[rstest]
    fn execute_naive(series_with_handles: SeriesWithHandles) {
        let series = &series_with_handles.series;
        series.execute_all(true).unwrap();
        let series_status = series.series_status().unwrap();
        assert_eq!(series_status.success, series.runs().len());
    }

    #[rstest]
    fn execute_failing(
        series_with_handles: SeriesWithHandles,
        #[values(true, false)] failfast: bool,
    ) {
        let series = &series_with_handles.series_fails;
        assert_ne!(series.runs().len(), 1);

        // must fail
        series.execute_all(failfast).unwrap_err();

        let status = series.series_status().unwrap();
        assert_eq!(series.runs().len(), status.total());

        let expected_failed = match failfast {
            true => 1,
            false => series.runs().len(),
        };
        let expected_not_started = match failfast {
            true => series.runs().len() - 1,
            false => 0,
        };

        assert_eq!(expected_failed, status.failed);
        assert_eq!(expected_not_started, status.not_started);
    }

    #[rstest]
    fn execute_not_started(series_with_handles: SeriesWithHandles) {
        let series = &series_with_handles.series;
        series.runs().first().unwrap().execute().unwrap();

        todo!("execute a simple series");
    }

    #[rstest]
    fn execute_all_some_started(series_with_handles: SeriesWithHandles) {
        let series = &series_with_handles.series;
        series.runs().first().unwrap().execute().unwrap();

        let expected_status = SeriesStatus {
            success: 1,
            not_started: series.runs().len() - 1,
            ..SeriesStatus::default()
        };

        assert_eq!(expected_status, series.series_status().unwrap());

        // do not start if some are started, even if non-started remain
        let err = series.execute_all(false).unwrap_err();
        assert!(err.to_string().contains("started"));

        // nothing changed in state
        assert_eq!(expected_status, series.series_status().unwrap());
    }
}
