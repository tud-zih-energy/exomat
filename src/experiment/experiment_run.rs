use super::experiment_traits::{FileReader, FileWriter, Runner};
use crate::experiment::out_file::{Observation, OutFile, OutList};
use crate::harness::env::{Environment, ExomatEnvironment};

use crate::helper::{
    archivist::{create_harness_dir, create_harness_file},
    errors::{Error, Result},
    fs_names::*,
};

use log::warn;
use log::{debug, error, info, trace};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Describes the current state of an Experiment Run
#[derive(Clone, Debug, PartialEq)]
pub enum RunStatus {
    /// Run hsn't been executed yet
    NotStarted,
    /// Run finished, and did not produce errors
    Success,
    /// Run finished, but exited with non-zero exit code
    ExitNonZero(u8),
}

/// Container for an Experiment Run
///
/// An Experiment Run must be written to the filesystem before it can be exeucted.
/// A normal workflow would be:
/// 1. `create(...)`
/// 2. `execute()`
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentRun {
    /// location on disk
    pub dir: PathBuf,
}

impl ExperimentRun {
    /// create a new run dir at `run_dir`
    ///
    /// use given template & env
    pub fn create(template: &Path, env: &Environment, run_dir: PathBuf) -> Result<Self> {
        if run_dir.exists() {
            return Err(Error::HarnessCreateError {
                entry: run_dir.to_string_lossy().to_string(),
                reason: "dir already exists".to_string(),
            });
        }

        fn cp_recursive_content(src_dir: &Path, dst_dir: &Path) -> Result<()> {
            for entry in std::fs::read_dir(src_dir)? {
                let entry = entry?;
                let entry_src = entry.path();
                let entry_dst = dst_dir.join(entry.file_name());

                if entry_src.is_dir() {
                    trace!("traversing into {}", entry_src.display());
                    std::fs::create_dir(&entry_dst)?;
                    cp_recursive_content(&entry_src, &entry_dst)?;
                } else if entry_src.is_file() || entry_src.is_symlink() {
                    trace!("copying {} to {}", entry_src.display(), entry_dst.display());
                    std::fs::copy(&entry_src, &entry_dst)?;
                }
            }

            Ok(())
        }

        std::fs::create_dir(&run_dir).map_err(|err| Error::HarnessCreateError {
            entry: run_dir.to_string_lossy().to_string(),
            reason: format!("failed to create run dir: {err}"),
        })?;

        debug!("copying template");
        cp_recursive_content(template, &run_dir)?;

        if !run_dir.join(RUN_RUN_FILE).is_file() {
            warn!(
                "{RUN_RUN_FILE} missing in {}, will error when executed",
                run_dir.display()
            );
        }

        debug!("writing environment");
        env.to_file(&run_dir.join(RUN_ENV_FILE))?;

        create_harness_file(&run_dir.join(MARKER_RUN))?;

        Ok(Self { dir: run_dir })
    }

    /// path to run dir on disk
    pub fn location(&self) -> &Path {
        self.dir.as_path()
    }

    /// read execution status from disk
    pub fn status(&self) -> Result<RunStatus> {
        let start_timestamp_file = self.location().join(RUN_OUT_EXOMAT_START_TIMESTAMP);
        let exit_code_file = self.location().join(RUN_OUT_EXOMAT_RUN_EXIT_CODE);

        match start_timestamp_file.is_file() {
            false => Ok(RunStatus::NotStarted),
            true => match std::fs::read_to_string(exit_code_file)?.parse::<u8>() {
                Ok(0) => Ok(RunStatus::Success),
                Ok(exit_code) => Ok(RunStatus::ExitNonZero(exit_code)),
                Err(err) => Err(Error::HarnessRunError {
                    experiment: self.run_name().to_string(),
                    err: format!(
                        "stored exit code (in {RUN_OUT_EXOMAT_RUN_EXIT_CODE}) is not a valid exit code (unsigned 8 bit integer): {err}"
                    ),
                }),
            },
        }
    }

    /// read out_ files from disk
    pub fn out_files(&self) -> Result<OutList> {
        OutList::parse(self.location())
    }

    /// read environment from disk
    pub fn env(&self) -> Result<Environment> {
        Environment::load_from_file(self.location())
    }

    /// shorthand to get basename of location
    pub fn run_name(&self) -> &str {
        self.location()
            .file_name()
            .expect("basename of dir must be available")
            .to_str()
            .expect("basename contains non-utf8 characters, wtf?!")
    }
}

// ========================== Runner ==========================
impl Runner for ExperimentRun {
    type Item = ();

    /// Executes [RUN_RUN_FILE] script found in `run_folder`.
    ///
    /// 1. read envs from `run_folder/RUN_ENV_FILE`
    /// 2. add `exomat_envs` (overwrites envs with the same name)
    /// 3. run `run_folder/RUN_RUN_FILE` with these envs
    /// 4. log run results
    ///     - Exomat output will **not** automatically be duplicated to the log file
    ///       by calling this function.
    ///
    /// ## Errors and Panics
    /// - Returns a `HarnessRunError` if the run has not been serialized yet
    /// - Returns a `HarnessRunError` if the script could not be executed
    /// - Returns a `HarnessRunError` if there is no [RUN_RUN_FILE] in `run_folder`
    /// - Returns a `HarnessRunError` if there is no [RUN_ENV_FILE] in `run_folder`
    fn execute(&mut self, exp_name: &str) -> Result<Self::Item> {
        todo!("refactor");
        // debug!("{exp_name}: Checking run directory {}", self.run_name);
        // trace!("checking if run has been serialized");
        // let run_folder = self
        //     .location
        //     .as_ref()
        //     .unwrap()
        //     .canonicalize()
        //     .map_err(|e| Error::HarnessRunError {
        //         experiment: exp_name.to_string(),
        //         err: format!("Experiment Run has not been written to disk yet: {e}"),
        //     })?;
        //
        // trace!("checking if all files exist in run");
        // for file in REQUIRED_RUN_FILES {
        //     if !run_folder.join(file).is_file() {
        //         return Err(Error::HarnessRunError {
        //             experiment: exp_name.to_string(),
        //             err: format!("Missing {file} in experiment run directory"),
        //         });
        //     };
        // }
        //
        // let run_name = run_folder
        //     .file_stem()
        //     .expect("run folder name inaccessable")
        //     .display()
        //     .to_string();
        // let exp_name_full = format!("{exp_name} {run_name}");
        //
        // debug!("{exp_name_full}: reading run environment");
        // let mut all_envs = self.exomat_env.to_environment_full();
        // all_envs.extend_envs(&self.env);
        //
        // debug!("{exp_name_full}: Starting execution of {}", self.run_name);
        //
        // // execute command with envs and collect any output in child
        // let mut run_command = Command::new(run_folder.join(RUN_RUN_FILE));
        // run_command.envs(all_envs).current_dir(&run_folder);
        // // automatically wrap stderr/stdout into logs
        // let run_output =
        //     crate::helper::logging::run_cmd_forwards_output(exp_name_full.as_str(), run_command)?;
        //
        // trace!("{exp_name_full}: Finished run {}", run_folder.display());
        // self.status = match run_output.status.success() {
        //     true => RunStatus::Success,
        //     false => RunStatus::Fail(run_output.status.to_string()),
        // };
        // trace!("stored status: {:?}", self.status());
        //
        // let exit_status = run_output.status;
        // match run_output.status.success() {
        //     true => {
        //         info!("{exp_name_full} finished successfully with {exit_status}");
        //         Ok(())
        //     }
        //     false => {
        //         error!("{exp_name_full} finished with non-zero {exit_status}");
        //         Err(Error::HarnessRunError {
        //             experiment: run_name.to_string(),
        //             err: "exit status non-zero, consider reviewing logs".to_string(),
        //         })
        //     }
        // }
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
    fn parse(exp_run_dir: &Path) -> Result<Self::Item> {
        debug!("reading environment");
        let env = Environment::try_from(&exp_run_dir.join(RUN_ENV_FILE)).unwrap_or_else(|_| {
            warn!("No environment found in run {}", exp_run_dir.display());
            Environment::new()
        });

        debug!("reading run script");
        let run_sh = std::fs::read_to_string(exp_run_dir.join(RUN_RUN_FILE))?;

        trace!("Reading out_ files of Run {}", exp_run_dir.display());
        let mut out_list: OutList = OutList::default();
        let contained_files = <ExperimentRun as FileReader>::find_all_files(exp_run_dir);

        for file in contained_files {
            trace!("checking file {}", file.display());
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
            true => out_list,
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
                        outfile.repeat(0, to_extend)?;

                        // We got multiple values for var, check if it has the same number of rows as the
                        // other columns
                    } else if len != max_length {
                        return Err(Error::EnvError {
                                        reason: format!("Mismatched number of values for {} {len}, other value in {} has {max_length}", outfile.var_name(), exp_run_dir.display())});
                    }
                }

                out_list
            }
        };

        debug!("creating exomat environment");
        let exomat_env = ExomatEnvironment::new(&PathBuf::new(), 1);
        let run_name = exp_run_dir
            .file_name()
            .expect("Could not parse run name")
            .display()
            .to_string();

        todo!();
        // Ok(ExperimentRun {
        //     run_sh,
        //     run_name,
        //     env,
        //     exomat_env,
        //     out_files: out_balanced,
        //     status: RunStatus::Unknown,
        //     location: Some(exp_run_dir.to_path_buf()),
        // })
    }
}

// ========================== Tests ==========================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::ExperimentRun;
    use crate::harness::env::Environment;
    use crate::test_fixtures::{env_1a, env_foo, template_dir};
    use rstest::{fixture, rstest};

    use tempfile::TempDir;

    #[fixture]
    fn run(template_dir: TempDir, env_foo: Environment) -> ExperimentRun {
        todo!("also emit TempDir handle, otherwise run dir will be destroyed as well");
        let new_dir = tempfile::TempDir::new().unwrap();
        let dst = new_dir.path().join("run_dir");
        ExperimentRun::create(template_dir.path(), &env_foo, dst.clone()).unwrap()
    }

    #[rstest]
    fn create_run(template_dir: TempDir, env_foo: Environment) {
        let new_dir = tempfile::TempDir::new().unwrap();
        let dst = new_dir.path().join("run_dir");

        let run = ExperimentRun::create(template_dir.path(), &env_foo, dst.clone()).unwrap();
        let dir = run.location();

        assert_eq!(dir.canonicalize().unwrap(), dst.canonicalize().unwrap());

        assert!(dir.is_dir());
        assert!(dir.join(MARKER_RUN).is_file());
        assert!(dir.join("run.sh").is_file());
        assert!(dir.join("file").is_file());
        assert!(dir.join("subdir").join("file").is_file());

        let env_from_file = Environment::load_from_file(&dir.join("environment.env")).unwrap();
        assert_eq!(env_from_file, env_foo);
    }

    #[rstest]
    fn create_run_dst_exists(template_dir: TempDir, env_foo: Environment) {
        let new_dir = tempfile::TempDir::new().unwrap();

        assert!(
            ExperimentRun::create(template_dir.path(), &env_foo, new_dir.path().to_path_buf())
                .is_err()
        );
    }

    #[rstest]
    fn run_env_set(mut run: ExperimentRun) {
        let debug_script = r#"#!/usr/bin/env bash
set -euo pipefail

echo $FOO > out_foo_val
"#;
        std::fs::write(run.location().join(RUN_RUN_FILE), debug_script).unwrap();

        run.execute("name").unwrap();

        let foo_file = run.location().join("out_foo_val");
        assert!(foo_file.is_file());
        assert_eq!("foo", std::fs::read_to_string(foo_file).unwrap());
    }

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(42)]
    fn run_status_dumped(mut run: ExperimentRun, #[case] return_code: u32) {
        let debug_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

exit {return_code}
"#
        );
        std::fs::write(run.location().join(RUN_RUN_FILE), debug_script).unwrap();

        let exp_started_file = run.location().join(RUN_OUT_EXOMAT_START_TIMESTAMP);
        let exp_done_file = run.location().join(RUN_OUT_EXOMAT_RUN_EXIT_CODE);

        assert!(!exp_started_file.is_file());
        assert!(!exp_done_file.is_file());

        run.execute("name").unwrap();

        assert!(exp_started_file.is_file());
        assert!(exp_done_file.is_file());

        todo!("check return code");
    }

    #[rstest]
    #[case(RunStatus::NotStarted)]
    #[case(RunStatus::Success)]
    #[case(RunStatus::ExitNonZero(42))]
    fn read_status_from_disk(
        run: ExperimentRun,
        #[values(true, false)] exp_started: bool,
        #[case] given_status: RunStatus,
    ) {
        let exp_started_file = run.location().join(RUN_OUT_EXOMAT_START_TIMESTAMP);
        let exp_done_file = run.location().join(RUN_OUT_EXOMAT_RUN_EXIT_CODE);

        if exp_started {
            std::fs::write(exp_started_file, "0").unwrap();
        }

        todo!("write exit status");

        // check status corectly read from disk
        let run: ExperimentRun = todo!("read exprun from disk");

        match (exp_started, given_status) {
            (false, RunStatus::NotStarted) => assert_eq!(given_status, run.status().unwrap()),
            (false, _) => assert!(run.status().is_err()),
            (true, RunStatus::NotStarted) => assert!(run.status().is_err()),
            (true, _) => assert_eq!(given_status, run.status().unwrap()),
        }
    }

    #[rstest]
    fn read_out_files(
        run: ExperimentRun,
        #[values(true, false)] out_a: bool,
        #[values(true, false)] out_b: bool,
    ) {
        if out_a {
            std::fs::write(run.location().join("out_a"), "foo").unwrap();
        }
        if out_b {
            std::fs::write(run.location().join("out_b"), "bar").unwrap();
        }

        let out_files = run.out_files().unwrap();

        let expected_len = out_a as usize + out_b as usize;
        assert_eq!(expected_len, out_files.len());

        if out_a {
            let found_out_a = out_files
                .iter()
                .find(|out_file| "a" == out_file.var_name())
                .unwrap();
            assert_eq!(vec!["foo".to_string()], *found_out_a.values());
        }

        if out_b {
            let found_out_a = out_files
                .iter()
                .find(|out_file| "b" == out_file.var_name())
                .unwrap();
            assert_eq!(vec!["bar".to_string()], *found_out_a.values());
        }
    }

    #[rstest]
    #[case(env_foo())]
    #[case(env_1a())]
    fn read_env(run: ExperimentRun, #[case] env: Environment) {
        env.to_file(run.location().join(RUN_ENV_FILE).as_path())
            .unwrap();

        assert_eq!(env, run.env().unwrap());
    }

    #[rstest]
    fn read_run_name(run: ExperimentRun) {
        assert_eq!(run.run_name(), run.location().file_name().unwrap());
    }
}
