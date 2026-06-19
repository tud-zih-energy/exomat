use super::experiment_traits::{FileReader, FileWriter, Runner};
use crate::experiment::out_file::{Observation, OutFile, OutList};
use crate::harness::env::{Environment, ExomatEnvironment};

use crate::helper::{
    archivist::{create_harness_dir, create_harness_file, find_all_files},
    errors::{Error, Result},
    fs_names::*,
};

use log::warn;
use log::{debug, error, info, trace};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};

/// Describes the current state of an Experiment Run
#[derive(Clone, Debug, PartialEq)]
pub enum RunStatus {
    /// Run hsn't been executed yet, status is unknown
    Unknown,
    /// Run didn't produce errors
    Success,
    /// Run produced an error
    Fail(String),
}

/// Container for an Experiment Run
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentRun {
    run_sh: String,
    run_name: String,
    env: Environment,
    exomat_env: ExomatEnvironment,
    out_files: Option<OutList>,
    status: RunStatus,
    location: Option<PathBuf>,
}

impl ExperimentRun {
    /// Creates a new Experiment Run
    ///
    /// The following values will be set:
    /// - `run_sh`: `run_sh`
    /// - `run_name`: built from `environment.0`, `exomat_environment.repetition` and `rep_format_length`
    /// - `env`: `environment.1`
    /// - `exomat_env`: `exomat_environment`
    /// - `out_files`: None
    /// - `status`: RunStatus::Unknown
    /// - `location`: None
    ///
    /// ## Panics
    /// - panics if `rep_format_length` is <= 0
    /// - panics if environment.1 contains reserved Environemnt variables (see ExomatEnvironment)
    pub fn new(
        run_sh: &str,
        environment: (&PathBuf, &Environment),
        exomat_environment: &ExomatEnvironment,
        rep_format_length: usize,
    ) -> Self {
        debug!("checking format length");
        assert!(rep_format_length > 0, "repetition format cannot be 0");

        debug!("checking envs for reserved env vars");
        assert!(!ExomatEnvironment::RESERVED_ENV_VARS
            .iter()
            .any(|k| environment.1.contains_env_var(k)));

        let dir_name = format!(
            "run_{}_rep{:0length$}",
            environment.0.file_prefix().unwrap().display(),
            exomat_environment.repetition,
            length = rep_format_length
        );

        trace!("Created new Experiment Run \"{dir_name}\"");
        Self {
            run_sh: run_sh.to_string(),
            run_name: dir_name,
            env: environment.1.clone(),
            exomat_env: exomat_environment.clone(),
            out_files: None,
            status: RunStatus::Unknown,
            location: None,
        }
    }

    /// Immutable iteration
    pub fn iter<'a>(&'a self) -> ExperimentRunIter<'a> {
        ExperimentRunIter {
            run_reader: self,
            index: 0,
        }
    }

    // ========================= getter ========================================

    /// Returns the run name
    pub fn run_dir_name(&self) -> &str {
        &self.run_name
    }

    /// Returns the current repetition
    pub fn repetition(&self) -> &u64 {
        &self.exomat_env.repetition
    }

    /// returns the environment of this Experiment Run
    pub fn environment(&self) -> &Environment {
        &self.env
    }

    /// Returns the run status
    pub fn status(&self) -> &RunStatus {
        &self.status
    }

    /// Returns the content of an out_ file `out_[var]`
    ///
    /// If there is no file with this name, `None` is returned.
    /// The returned Vec may be empty.
    pub fn out_var(&self, var: &str) -> Option<&Vec<String>> {
        match &self.out_files {
            Some(out) => {
                if let Some(outfile) = out.outfile(var) {
                    Some(outfile.values())
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Returns a reference to the list of out_ files recorded
    pub fn out_files(&self) -> &Option<OutList> {
        &self.out_files
    }

    // ========================= setter ========================================

    /// Replaces `self.out_files` with `new_out`.
    ///
    /// The content of `new_out` is not checked in any way.
    pub fn replace_out_files_unchecked(&mut self, new_out: Option<OutList>) {
        self.out_files = new_out
    }

    // ========================= helper ========================================

    /// Generates an ExperimentRun from `outlist`.
    ///
    /// Sets an empty Environemnt.
    #[cfg(test)]
    pub fn from_out_list_unchecked(outlist: &OutList) -> Self {
        ExperimentRun {
            run_sh: String::new(),
            run_name: TEST_RUN_REP_DIR0.to_string(),
            env: Environment::new(),
            exomat_env: ExomatEnvironment {
                exp_src_dir: PathBuf::new(),
                repetition: 1,
            },
            out_files: Some(outlist.clone()),
            status: RunStatus::Unknown,
            location: None,
        }
    }

    /// Helper, that returns the value at `index` for all keys in the current run.
    ///
    /// If a value is empty "NA" will be returned as the key's value.
    ///
    /// - Returns an `IndexOutOfRange` Error if the index is out of range (unbelievable, I know)
    /// - Returns an `Empty` Error if there are no Observations, that can be returned
    fn get_observation(&self, index: usize) -> Result<Observation> {
        debug!("looking for out_ files");
        if let Some(out_files) = &self.out_files {
            let mut observation: Observation = HashMap::new();
            for outfile in out_files.iter() {
                debug!("checking if out_ file is empty");
                if outfile.is_empty() {
                    observation.insert(outfile.var_name().clone(), String::from("NA"));
                    // index is not in range
                } else if index >= outfile.value_count() {
                    return Err(Error::IndexOutOfRange {
                        index,
                        limit: outfile.value_count(),
                    });
                    // everything worked, get value
                } else {
                    observation.insert(
                        outfile.var_name().clone(),
                        outfile.values()[index].to_string(),
                    );
                }
            }

            debug!("observation found: {:?}", observation);
            Ok(observation)

        // no out_files in this run
        } else {
            Err(Error::Empty(String::from("No Observations found")))
        }
    }

    /// Produce log output based on exit_status and err_log content.
    ///
    /// - exit_status:
    ///    - **success**  : log info
    ///    - **failed**   : log error (don't evaluate err_log after)
    /// - err_log:
    ///    - **empty**    : log info
    ///    - **not empty**: log warning
    ///
    /// ## Errors
    /// - Returns a HarnessRunError if `exit_status` shows a failure
    fn log_run_result(
        &self,
        run_name: &str,
        exit_status: std::process::ExitStatus,
        err_log: &String,
    ) -> Result<()> {
        if exit_status.success() {
            info!("{run_name} finished successfully with {exit_status}");

            if err_log.is_empty() {
                info!("{run_name} did not produce stderr output");
            } else {
                warn!("{run_name} produced stderr output");
            }
        } else {
            error!("{run_name} finished with non-zero {exit_status}");

            // fail fast in case of unsuccessful run
            return Err(Error::HarnessRunError {
                experiment: run_name.to_string(),
                err: err_log.clone(),
            });
        }

        Ok(())
    }
}

// ========================== Runner ==========================
impl Runner for ExperimentRun {
    type Item = (String, String);

    /// Executes [RUN_RUN_FILE] script found in `run_folder`.
    ///
    /// 1. read envs from `run_folder/RUN_ENV_FILE`
    /// 2. add `exomat_envs` (overwrites envs with the same name)
    /// 3. run `run_folder/RUN_RUN_FILE` with these envs
    /// 4. log run results
    ///     - Appends any stderr/stdout output into their respective log file in the
    ///       parent series directory of `run_folder`.
    ///     - Exomat output will **not** automatically be duplicated to the log file
    ///       by calling this function.
    ///
    /// ## Errors and Panics
    /// - Returns a `HarnessRunrror` if the run has not been serialized yet
    /// - Returns a `HarnessRunrror` if the script could not be executed
    /// - Returns a `HarnessRunrror` if there is no [RUN_RUN_FILE] in `run_folder`
    /// - Returns a `HarnessRunrror` if there is no [RUN_ENV_FILE] in `run_folder`
    fn execute(&mut self, exp_name: &str) -> Result<Self::Item> {
        trace!("{exp_name}: Checking run directory {}", self.run_name);
        debug!("checking if run has been serialized");
        let run_folder = self
            .location
            .as_ref()
            .unwrap()
            .canonicalize()
            .map_err(|e| Error::HarnessRunError {
                experiment: exp_name.to_string(),
                err: format!("Experiment Run has not been serialized yet: {e}"),
            })?;

        debug!("checking if all files exist in run");
        for file in [RUN_ENV_FILE, RUN_ENV_FILE] {
            if !run_folder.join(file).is_file() {
                return Err(Error::HarnessRunError {
                    experiment: exp_name.to_string(),
                    err: format!("Missing {file} in experiment run directory"),
                });
            };
        }

        debug!("reading run environment");
        let mut all_envs = self.exomat_env.to_environment_full();
        all_envs.extend_envs(&self.env);

        trace!("{exp_name}: Starting execution of {}", self.run_name);

        // execute command with envs and collect any output in child
        let run = Command::new(run_folder.join(RUN_RUN_FILE))
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .envs(all_envs.to_env_map())
            .current_dir(&run_folder)
            .output()
            .map_err(|e| Error::HarnessRunError {
                experiment: exp_name.to_string(),
                err: e.to_string(),
            })?;

        trace!("{exp_name}: Finished run {}", run_folder.display());
        debug!("reading logs");
        let stdout = String::from_utf8_lossy(&run.stdout).to_string();
        let stderr = String::from_utf8_lossy(&run.stderr).to_string();

        debug!("updating run status");
        match run.status.success() {
            true => self.status = RunStatus::Success,
            false => self.status = RunStatus::Fail(run.status.to_string()),
        };

        self.log_run_result(
            run_folder.file_stem().unwrap().to_str().unwrap(),
            run.status,
            &stderr,
        )?;

        Ok((stdout, stderr))
    }
}

// ========================== Writer ==========================
impl FileWriter for ExperimentRun {
    /// Creates a ready-to-use experiment run for **one interation** with **one environment**
    /// of an experiment.
    ///
    /// ## out_ files in `self.out_files` will not be created
    /// as they should created by the run script
    ///
    /// The new directory will be created in the given `dir`.
    /// This will result in the following structure:
    /// ```notest
    /// series_folder
    ///   |-> ...
    ///   \-> runs/
    ///     |-> ...
    ///     \-> dir
    ///       |-> .exomat_run
    ///       |-> RUN_RUN_FILE     (copy of SRC_RUN_FILE)
    ///       \-> RUN_ENV_FILE     (copy of env_file)
    /// ```
    ///
    /// If no Errors occured, the path to the created experiment run folder will be returned.
    ///
    /// ## Errors and Panics
    /// - Returns a `HarnessCreateError` if there is no [SERIES_RUNS_DIR] found inside `series_folder`
    /// - Returns a `HarnessCreateError` if any file or directory could not be created or copied
    /// - Panics if `it_format_length` is 0
    fn persist(&mut self, exp_run_dir: &PathBuf) -> Result<()> {
        create_harness_dir(&exp_run_dir)?;
        create_harness_file(&exp_run_dir.join(MARKER_RUN))?;

        debug!("copy ruh.sh and [env].env to runs_dir");
        let run_file_path = &exp_run_dir.join(RUN_RUN_FILE);
        OpenOptions::new()
            .mode(0o775)
            .write(true)
            .create_new(true)
            .open(run_file_path)
            .map_err(|e| Error::HarnessCreateError {
                entry: run_file_path.to_str().unwrap().to_string(),
                reason: e.to_string(),
            })?;

        std::fs::write(run_file_path, &self.run_sh)?;

        debug!("write envs to file (including exomat envs)");
        let mut serializable_envs = self.env.clone();
        serializable_envs.extend_envs(&self.exomat_env.to_environment_serializable());
        serializable_envs.to_file(&exp_run_dir.join(RUN_ENV_FILE))?;

        trace!("Persisted Experiment Run at {}", exp_run_dir.display());
        debug!("update run location");
        self.location = Some(exp_run_dir.to_path_buf());

        Ok(())
    }
}

// ========================== Display ==========================
impl std::fmt::Display for ExperimentRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Experiment Run \"{}\" at {:?}:\n    Run script: {}\n    Environment: {:?}\n    Internat envs: {:?}\n    Status: {:?}\n    contains out files: {:#?}",
            self.run_name,
            self.location,
            match self.run_sh.is_empty() {
                true => "not set",
                false => "set"
            },
            self.env,
            self.exomat_env.to_environment_full(),
            self.status,
            self.out_files,
        )
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentRun {
    type Item = ExperimentRun;

    /// Parses an Experiment Run directory into an ExperimentRun.
    ///
    /// Will balance out missing values, if possible, so that the number of values
    /// is even across all out_ files.
    ///
    /// The content of out_ files is not validated or checked in any way, if you put
    /// weird content in them, you will get weird output.
    ///
    /// ### Warnings, Errors and Panics
    /// What you will be **warn**ed about:
    /// - no env file at run/[RUN_ENV_FILE] (Empty Environment will be used)
    /// - an out_ file shadows an env var
    ///
    /// What will cause an **Error**:
    /// - invalid out_ file names
    /// - unbalanced multiline out_ files
    ///
    /// This function might **Panic** if reading/writing failed.
    fn parse(exp_run_dir: &PathBuf) -> Result<Self::Item> {
        debug!("reading environment");
        let env = Environment::from_file(&exp_run_dir.join(RUN_ENV_FILE)).unwrap_or_else(|_| {
            warn!("No environment found in run {}", exp_run_dir.display());
            Environment::new()
        });

        debug!("reading run script");
        let run_sh = std::fs::read_to_string(&exp_run_dir.join(RUN_RUN_FILE))?;

        trace!("Reading out_ files of Run {}", exp_run_dir.display());
        let mut out_list: OutList = OutList::default();
        let contained_files = find_all_files(&exp_run_dir)?;

        for file in contained_files {
            debug!("checking file {}", file.display());
            match OutFile::parse(&file) {
                Err(Error::Empty(e)) => return Err(Error::Empty(e)), // this means the name is invalid
                Err(_) => continue,
                Ok(outfile) => {
                    // warn if out file shadows env var
                    if env.contains_env_var(outfile.var_name()) {
                        warn!(
                            "in {}: out_{} shadows input environment variable ${}",
                            outfile.var_name(),
                            exp_run_dir.display(),
                            outfile.var_name(),
                        );
                    }

                    // extend existing outlist
                    if out_list.contains(&outfile) {
                        let to_extend = out_list
                            .iter_mut()
                            .find(|f| f.var_name() == outfile.var_name())
                            .expect("Could not locate out file to append to");

                        to_extend.extend_values(outfile.values());
                    } else {
                        out_list.push(outfile);
                    }
                }
            }
        }

        // balance values
        trace!("Balancing out_ files of Run {}", exp_run_dir.display());
        let out_balanced = match out_list.is_empty() {
            true => None,
            false => {
                let max_length = out_list
                    .iter()
                    .map(|out| out.value_count())
                    .max()
                    .unwrap_or(1);

                // for each variable
                for outfile in out_list.iter_mut() {
                    let len = outfile.value_count();

                    if len == 1 && max_length > 1 {
                        let to_extend = max_length - len;
                        outfile.repeat(0, to_extend);

                        // We got multiple values for var, check if it has the same number of rows as the
                        // other columns
                    } else if len != max_length {
                        return Err(Error::EnvError {
                                        reason: format!("Mismatched number of values for {} {len}, other value in {} has {max_length}", outfile.var_name(), exp_run_dir.display())});
                    }
                }

                Some(out_list)
            }
        };

        debug!("creating exomat environment");
        let exomat_env = ExomatEnvironment::new(&PathBuf::new(), 1);
        let run_name = exp_run_dir
            .file_name()
            .expect("Could not parse run name")
            .display()
            .to_string();

        Ok(ExperimentRun {
            run_sh,
            run_name,
            env,
            exomat_env,
            out_files: out_balanced,
            status: RunStatus::Unknown,
            location: Some(exp_run_dir.to_path_buf()),
        })
    }
}

// ========================== Iterator ==========================
/// Iterator for RunReader
///
/// Iterates over the Observations in an EXperiment Run.
#[derive(Debug)]
pub struct ExperimentRunIter<'a> {
    run_reader: &'a ExperimentRun,
    index: usize,
}

impl<'a> Iterator for ExperimentRunIter<'a> {
    type Item = Observation;

    fn next(&mut self) -> Option<Self::Item> {
        let obs = self.run_reader.get_observation(self.index);
        self.index += 1;

        match obs {
            Ok(obs) => Some(obs),
            Err(_) => None,
        }
    }
}
impl<'a> IntoIterator for &'a ExperimentRun {
    type Item = Observation;
    type IntoIter = ExperimentRunIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ========================== Tests ==========================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::{ExperimentRun, ExperimentSeries, ExperimentSource, FileWriter};
    use crate::harness::env::Environment;
    use crate::helper::test_fixtures::{setup_run_dir, setup_run_dir_shadow, setup_series_no_out};
    use crate::helper::test_helper::populate_src_with_series;

    use tempfile::TempDir;

    #[test]
    fn build_run_directory_simple() {
        // create base tempdir, to act as parent
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path();

        // create an experiment source, and an experiment series
        let src_name = "FooSource";
        let ser_name = "FooSeries";
        let (_, mut ser) = populate_src_with_series(&tmpdir.to_path_buf(), src_name, ser_name);

        // create Experiment Run in ser, equals to one repetition of one environment)
        ser.generate_runs().unwrap();
        assert_eq!(ser.runs().len(), 1);
        ser.persist(&tmpdir.join(ser_name)).unwrap();

        let runs_dir = ser.location().as_ref().unwrap().join(SERIES_RUNS_DIR);
        let run_dir = runs_dir.join("run_0_rep0");
        assert!(run_dir.is_dir());
        assert!(run_dir.join(RUN_ENV_FILE).is_file());
        assert!(run_dir.join(RUN_RUN_FILE).is_file());
        // assert!(run_dir.join(RUN_RUN_FILE).executable());

        // check that exomat envs are included (or not)
        let envs = Environment::from_file(&run_dir.join(RUN_ENV_FILE)).unwrap();
        assert_eq!(envs.get_env_val("REPETITION"), Some(&String::from("0")));
        assert_eq!(envs.get_env_val("EXP_SRC_DIR"), None);
    }

    #[test]
    fn test_run_repetition_format() {
        // create base tempdir, to act as parent
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path();

        // create an experiment source and set repetition to something higher, to get leading zeros in directory names
        let mut src = ExperimentSource::new();
        src.set_exomat_envs(ExomatEnvironment {
            exp_src_dir: tmpdir.join("FooSource"),
            repetition: 15,
        });
        src.persist(&tmpdir.join("FooSource")).unwrap();

        let mut ser = ExperimentSeries::from_source(&src).unwrap();
        ser.generate_runs().unwrap();
        assert_eq!(ser.runs().len(), 15);
        ser.persist(&tmpdir.to_path_buf()).unwrap();

        let runs_dir = ser.location().as_ref().unwrap().join(SERIES_RUNS_DIR);
        assert!(runs_dir.join("run_0_rep00").is_dir());
        assert!(runs_dir.join("run_0_rep14").is_dir());
        assert!(!runs_dir.join("run_0_rep15").is_dir());
    }

    #[test]
    fn test_internal_envs_not_in_files() {
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path();
        let source_name = "FooSource";
        let series_name = "FooSeries";

        // create a source with all needed envs set
        let mut src = ExperimentSource::new();
        src.set_exomat_envs(ExomatEnvironment::new(&tmpdir.join(source_name), 1));
        src.set_envs(HashMap::from([(
            PathBuf::from(SRC_ENV_FILE),
            Environment::from_env_list(vec![("FOO".to_string(), "bar".to_string())]),
        )]))
        .unwrap();
        src.persist(&tmpdir.join(source_name)).unwrap();

        // create a series based on this source
        let mut ser = ExperimentSeries::from_source(&src).unwrap();
        ser.generate_runs().unwrap();
        assert_eq!(ser.runs().len(), 1);
        ser.persist(&tmpdir.join(series_name)).unwrap();

        // check contents of env files
        let src_env =
            Environment::from_file(&src.location().join(SRC_ENV_DIR).join(SRC_ENV_FILE)).unwrap();
        let run_env = Environment::from_file(
            &ser.location()
                .as_ref()
                .unwrap()
                .join(SERIES_RUNS_DIR)
                .join("run_0_rep0")
                .join(RUN_ENV_FILE),
        )
        .unwrap();

        // exomat variable, never serialized
        assert!(!src_env.contains_env_var("EXP_SRC_DIR"));
        assert!(!run_env.contains_env_var("EXP_SRC_DIR"));

        // exomat variable, serialized
        assert!(!src_env.contains_env_var("REPETITION"));
        assert!(run_env.contains_env_var("REPETITION"));

        // user variable, always serialized
        assert!(src_env.contains_env_var("FOO"));
        assert!(run_env.contains_env_var("FOO"));
    }

    #[test]
    fn runreader_iter_working() {
        let tmp_run = setup_run_dir();
        let runs_dir = tmp_run.path().to_path_buf();

        let run_reader = ExperimentRun::parse(&runs_dir).unwrap();
        let mut run_iter = run_reader.iter();

        // iterate over runs, should work
        let obs1 = run_iter.next().unwrap();
        assert_eq!(obs1.get("number").unwrap(), "1");
        assert_eq!(obs1.get("word").unwrap(), "one");

        let obs2 = run_iter.next().unwrap();
        assert_eq!(obs2["number"], "2");
        assert_eq!(obs2["word"], "two");

        assert!(run_iter.next().is_none());
    }

    #[test]
    fn runreader_empty_out_files() {
        let tmp = setup_series_no_out();
        let series = tmp.path().to_path_buf();

        // iterator is created, but no observations
        let run_reader =
            ExperimentRun::parse(&series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0)).unwrap();
        let mut run_iter = run_reader.iter();
        println!("{run_iter:?}");
        assert!(run_iter.next().is_none());
    }

    #[test]
    fn runreader_out_file_shadows_env_var() {
        let tmp = setup_run_dir_shadow();
        let run_dir = tmp.path().to_path_buf();

        // Should not panic, but log a warning
        let run_reader = ExperimentRun::parse(&run_dir).unwrap();
        let mut iter = run_reader.iter();

        let obs = iter.next().unwrap();
        assert_eq!(obs.get("VAR1").unwrap(), "1");
        assert_eq!(obs.get("word").unwrap(), "one");
    }
}
