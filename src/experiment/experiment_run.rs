use super::experiment_traits::{FileReader, FileWriter, Runner};
use crate::experiment::out_file::{Observation, OutFile, OutList};
use crate::harness::env::{Environment, ExomatEnvironment};

use crate::helper::{
    archivist::{create_harness_file, find_all_files},
    errors::{Error, Result},
    fs_names::*,
};

use log::warn;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum RunStatus {
    Unknown,
    Ready,
    Success,
    Fail(String),
}

/// Container for an Experiment Run
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentRun {
    run_sh: String,
    env: Environment,
    out_files: Option<OutList>,
    status: RunStatus,
    location: Option<PathBuf>,
}

impl ExperimentRun {
    pub fn new(run_sh: &str, environment: &Environment) -> Self {
        // assert that all exomat env vars are added
        assert!(ExomatEnvironment::RESERVED_ENV_VARS
            .iter()
            .all(|k| environment.contains_env_var(k)));

        Self {
            run_sh: run_sh.to_string(),
            env: environment.clone(),
            out_files: None,
            status: RunStatus::Ready,
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

    pub fn environment(&self) -> &Environment {
        &self.env
    }

    /// Returns the content of an out_ file `out_[var]`
    ///
    /// If there is no file with this name, `None` is returned.
    /// The returned Vec may be empty.
    pub fn get_var(&self, var: &str) -> Option<&Vec<String>> {
        match &self.out_files {
            Some(out) => {
                if let Some(outfile) = out.get_outfile(var) {
                    Some(outfile.values())
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Returns a reference to the list of out_ files recorded
    pub fn get_out_files(&self) -> &Option<OutList> {
        &self.out_files
    }

    /// Replaces `self.out_files` with `new_out`.
    ///
    /// The content of `new_out` is not checked in any way.
    pub fn replace_out_files_unchecked(&mut self, new_out: Option<OutList>) {
        self.out_files = new_out
    }

    /// Generates a RunReader from `outlist`.
    ///
    /// Sets an empty Environemnt.
    pub fn from_out_list_unchecked(outlist: &OutList) -> Self {
        ExperimentRun {
            run_sh: String::new(),
            env: Environment::new(),
            out_files: Some(outlist.clone()),
            status: RunStatus::Ready,
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
        // there are out_files
        if let Some(out_files) = &self.out_files {
            let mut observation: Observation = HashMap::new();
            for outfile in out_files.iter() {
                // no values recorded
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

use log::{error, info, trace};
use std::process::{Command, Stdio};

// ========================== Runner ==========================
impl Runner for ExperimentRun {
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
    /// - Returns a `HarnessRunrror` if [RUN_RUN_FILE] could not be executed
    /// - panics if there is no [RUN_RUN_FILE] in `run_folder`
    /// - panics if there is no [RUN_ENV_FILE] in `run_folder`
    fn execute(&mut self, exp_name: String) -> Result<(String, String)> {
        // assert!(
        //     self.join(RUN_RUN_FILE).is_file(),
        //     "Missing run.sh in experiment run directory"
        // );

        // assert!(
        //     run_folder.join(RUN_ENV_FILE).is_file(),
        //     "Missing environment.env in experiment run directory"
        // );

        if self.location.is_none() {
            return Err(Error::HarnessRunError {
                experiment: exp_name,
                err: String::from("Experiment Run has not been serialized yet. Cannot execute."),
            });
        }
        let run_folder = self.location.as_ref().unwrap().canonicalize().unwrap();

        trace!(
            "{exp_name}: Starting execution of {}",
            run_folder.file_stem().unwrap().to_str().unwrap()
        );

        // execute command with envs and collect any output in child
        let run = Command::new(&self.run_sh)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .envs(self.env.to_env_map())
            .current_dir(&run_folder)
            .output()
            .map_err(|e| Error::HarnessRunError {
                experiment: exp_name.to_string(),
                err: e.to_string(),
            })?;

        trace!("{exp_name}: Finished run {}", run_folder.display());

        // write to logs
        let stdout = String::from_utf8(run.stdout).map_err(|e| Error::HarnessRunError {
            experiment: exp_name.clone(),
            err: e.to_string(),
        })?;
        let stderr = String::from_utf8(run.stderr).map_err(|e| Error::HarnessRunError {
            experiment: exp_name.clone(),
            err: e.to_string(),
        })?;

        // set run status
        match run.status.success() {
            true => self.status = RunStatus::Success,
            false => self.status = RunStatus::Fail(run.status.to_string()),
        };

        // write to console
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
    /// Creates a ready-to-use experiment run folder for **one interation** with **one environment**
    /// of an experiment.
    ///
    /// ### Note: `env_file` is used to deduce the `{env}` part of the new experiment run directory name.
    /// ###       `exomat_environment` is used to get the `{it}` part.
    ///
    /// The new directory will be created in the given `series_folder` under [SERIES_RUNS_DIR]`/run_[env]_rep[repetition]`.
    /// This will result in the following structure:
    /// ```notest
    /// series_folder
    ///   |-> ...
    ///   \-> runs/
    ///     |-> ...
    ///     \-> run_{env}_rep{it}/
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
    fn persist(&mut self, dir: &PathBuf) -> Result<()> {
        // assert!(it_format_length > 0, "repetition format cannot be 0");

        // // unwrap here, because this should never fail and if it does it's your fault
        // let env_name = &env_file.file_stem().unwrap().to_str().unwrap();

        // let run = format!(
        //     "run_{}_rep{:0length$}",
        //     env_name,
        //     exomat_environment.repetition,
        //     length = it_format_length,
        // );

        // get path to runs/, return error if it does not exist
        // let runs_dir = match series_folder.join(SERIES_RUNS_DIR).is_dir() {
        //     true => series_folder.join(SERIES_RUNS_DIR),
        //     false => {
        //         return Err(Error::HarnessCreateError {
        //             entry: run,
        //             reason: format!(
        //                 "{} dir does not exist in {}",
        //                 SERIES_RUNS_DIR,
        //                 series_folder.display()
        //             ),
        //         })
        //     }
        // };

        // let run = create_harness_dir(&runs_dir.join(run))?;

        create_harness_file(&dir.join(MARKER_RUN))?;

        // copy ruh.sh and [env].env to runs_dir
        let run_file = create_harness_file(&dir.join(RUN_RUN_FILE))?;
        std::fs::write(run_file, &self.run_sh)?;
        self.env.to_file(&dir.join(RUN_ENV_FILE))?;

        self.location = Some(dir.to_path_buf());

        Ok(())
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentRun {
    type Item = ExperimentRun;

    /// Parses an Experiment Run into a RunReader object.
    ///
    /// Will balance out missing values, if possible, so that the number of values
    /// is even across all out_ files.
    ///
    /// The content of out_ files is not validated or checked in any way, if you put
    /// weird content in them, you will get weird output.
    ///
    /// ### Warnings, Errors and Panics
    /// What you will be **warn**ed about:
    /// - no env file at run/[RUN_ENV_FILE]
    /// - an out_ file shadows an env var
    ///
    /// What will cause an **Error**:
    /// - invalid out_ file names
    /// - unbalanced multiline out_ files
    ///
    /// This function my **Panic** if reading/writing failed.
    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        // read env file
        let env = Environment::from_file(&dir.join(RUN_ENV_FILE)).unwrap_or_else(|_| {
            warn!("No environment found in run {}", dir.display());
            Environment::new()
        });

        // read out_file
        let run_sh = std::fs::read_to_string(&dir.join(RUN_RUN_FILE))?;

        // read out files
        let mut out_list: OutList = OutList::default();
        let contained_files = find_all_files(&dir)?;

        for file in contained_files {
            match OutFile::parse(&file) {
                Err(Error::Empty(e)) => return Err(Error::Empty(e)), // this means the name is invalid
                Err(_) => continue,
                Ok(outfile) => {
                    // warn if out file shadows env var
                    if env.contains_env_var(outfile.var_name()) {
                        warn!(
                            "in {}: out_{} shadows input environment variable ${}",
                            outfile.var_name(),
                            dir.display(),
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
                                        reason: format!("Mismatched number of values for {} {len}, other value in {} has {max_length}", outfile.var_name(), dir.display())});
                    }
                }

                Some(out_list)
            }
        };

        Ok(ExperimentRun {
            run_sh,
            env: env,
            out_files: out_balanced,
            status: RunStatus::Unknown,
            location: Some(dir.to_path_buf()),
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
    use crate::experiment::{ExperimentRun, ExperimentSeries, FileWriter};
    use crate::harness::env::Environment;
    use crate::helper::test_fixtures::{setup_run_dir, setup_run_dir_shadow, setup_series_no_out};
    use crate::helper::test_helper::populate_src_with_series;

    use faccess::PathExt;
    use tempfile::TempDir;

    #[test]
    fn build_run_directory_simple() {
        // create base tempdir, to act as parent
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path();

        // create an experiment source, and an experiment series
        let (mut src, mut ser) =
            populate_src_with_series(&tmpdir.to_path_buf(), "FooSource", "FooSeries");

        // create Experiment Run in ser, equals to one repetition of one environment)
        ser.generate_runs();
        assert_eq!(ser.repetition_count(), 1);
        ser.persist(&tmpdir.to_path_buf()).unwrap();

        let runs_dir = tmpdir.join("FooSeries").join(SERIES_RUNS_DIR);
        let run_dir = runs_dir.join("run_0_rep1");
        assert!(run_dir.is_dir());
        assert!(run_dir.join(RUN_ENV_FILE).is_file());
        assert!(run_dir.join(RUN_RUN_FILE).is_file());
        assert!(run_dir.join(RUN_RUN_FILE).executable());

        // check that exomat envs are included
        let envs = Environment::from_file(&run_dir.join(RUN_ENV_FILE)).unwrap();
        assert_eq!(envs.get_env_val("REPETITION"), Some(&String::from("1")));
        assert_eq!(
            envs.get_env_val("EXP_SRC_DIR"),
            Some(&src.location().display().to_string())
        );

        // set repetition to something higher, to get leading zeros in directory names
        src.set_exomat_envs(ExomatEnvironment {
            exp_src_dir: src.location().to_path_buf(),
            repetition: 15,
        });
        let mut ser = ExperimentSeries::from_source(&src);
        ser.generate_runs();
        assert_eq!(ser.repetition_count(), 15);
        ser.persist(&tmpdir.to_path_buf()).unwrap();

        assert!(!runs_dir.join("run_0_rep00").is_dir());
        assert!(runs_dir.join("run_0_rep01").is_dir());
        assert!(runs_dir.join("run_0_rep15").is_dir());
        assert!(!runs_dir.join("run_0_rep16").is_dir());
    }

    #[test]
    fn test_internal_envs_not_in_files() {
        // set up source/series/run dir
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path();
        let (src, mut ser) =
            populate_src_with_series(&tmpdir.to_path_buf(), "FooSource", "FooSeries");

        ser.generate_runs();
        assert_eq!(ser.repetition_count(), 1);

        // check contents of env files
        let src_env =
            Environment::from_file(&src.location().join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE))
                .unwrap();
        let run_env = Environment::from_file(
            &tmpdir
                .join("FooSeries")
                .join(SERIES_RUNS_DIR)
                .join("run_0_rep1")
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
