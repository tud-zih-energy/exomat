use super::experiment_traits::{FileReader, FileWriter, Runner};
use super::ExperimentSource;
use crate::experiment::out_file::{Observation, OutFile, OutList};
use crate::harness::env::{Environment, ExomatEnvironment, ENV_NAME_EXP_SOURCE_DIR};

use crate::helper::{
    archivist::{create_harness_dir, create_harness_file},
    errors::{Error, Result},
    fs_names::*,
};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, error, info, trace, warn};

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
    pub fn create(source: &ExperimentSource, env: &Environment, run_dir: PathBuf) -> Result<Self> {
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
        cp_recursive_content(source.template().as_path(), &run_dir)?;

        if !run_dir.join(RUN_RUN_FILE).is_file() {
            warn!(
                "{RUN_RUN_FILE} missing in {}, will error when executed",
                run_dir.display()
            );
        }

        debug!("writing environment");
        env.to_file(&run_dir.join(RUN_ENV_FILE))?;

        debug!("linking source");
        let path_to_source_relative_to_run_dir: PathBuf =
            pathdiff::diff_paths(source.location(), &run_dir).ok_or(Error::HarnessCreateError {
                entry: run_dir.to_string_lossy().to_string(),
                reason: "could not compute relative path to source, wtf?".to_string(),
            })?;
        std::os::unix::fs::symlink(
            path_to_source_relative_to_run_dir,
            run_dir.join(RUN_SOURCE_SYMLINK_NAME),
        )?;

        debug!("done, adding marker file");
        let _ = create_harness_file(&run_dir.join(MARKER_RUN))?;

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
        let path = self.location().join(RUN_ENV_FILE);
        Environment::try_from(&path)
    }

    /// shorthand to get basename of location
    pub fn run_name(&self) -> &str {
        self.location()
            .file_name()
            .expect("basename of dir must be available")
            .to_str()
            .expect("basename contains non-utf8 characters, wtf?!")
    }

    /// Executes [RUN_RUN_FILE] script found in `run_dir`.
    ///
    /// 1. read envs from `run_dir/RUN_ENV_FILE`
    /// 2. add env var pointing to absolute path of symlinked experiment source.
    /// 3. run `run_dir/RUN_RUN_FILE` with these envs
    ///
    /// Stdout & stderr are forwarded to logs. Exomat output will **not** automatically be
    /// duplicated to the log file by calling this function.
    ///
    /// Errors if the execution itself failed.
    /// If the run script exited non-zero, Ok(...) is returned (!!)
    pub fn execute(&mut self) -> Result<RunStatus> {
        debug!("Checking run directory {}", self.dir.display());

        // fail simple errors early
        trace!("checking if all files exist in run");
        for file in [MARKER_RUN, RUN_ENV_FILE, RUN_RUN_FILE] {
            if !self.dir.join(file).is_file() {
                return Err(Error::HarnessRunError {
                    experiment: self.run_name().to_string(),
                    err: format!("Missing {file} in experiment run directory"),
                });
            };
        }

        if self.dir.join(RUN_OUT_EXOMAT_START_TIMESTAMP).is_file() {
            return Err(Error::HarnessRunError {
                experiment: self.run_name().to_string(),
                err: format!("run has already been started before, aborting"),
            });
        }

        // execute command with envs and collect any output in child
        let mut run_command = Command::new(self.dir.join(RUN_RUN_FILE));

        trace!("preparing env & loading full path to source");
        let mut env = self.env()?;
        env.add_env(
            ENV_NAME_EXP_SOURCE_DIR.to_string(),
            self.location()
                .join(RUN_SOURCE_SYMLINK_NAME)
                .canonicalize()?
                .to_string_lossy()
                .to_string(),
        );

        trace!("preparing run invocation (env vars & pwd)");
        run_command.envs(env).current_dir(&self.dir);

        trace!("dumping start timestamp");
        std::fs::write(
            self.dir.join(RUN_OUT_EXOMAT_START_TIMESTAMP),
            chrono::Utc::now().timestamp().to_string(),
        )?;

        debug!("Starting execution of {}", self.run_name());

        // automatically wrap stderr/stdout into logs
        let run_output =
            crate::helper::logging::run_cmd_forwards_output(self.run_name(), run_command)?;

        trace!("Finished execution of {}", self.run_name());

        if let Some(exit_code) = run_output.status.code() {
            debug!("dumping exit code");
            std::fs::write(
                self.dir.join(RUN_OUT_EXOMAT_RUN_EXIT_CODE),
                format!("{exit_code}"),
            )?;
        } else {
            return Err(Error::HarnessRunError {
                experiment: self.run_name().to_string(),
                err: format!("exit code of run not available (wtf?!)"),
            });
        }

        let exit_status = run_output.status;
        let run_name = self.run_name();
        match run_output.status.success() {
            true => info!("{run_name} finished successfully with {exit_status}"),
            false => error!("{run_name} finished with non-zero {exit_status}"),
        };
        self.status()
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
    /// Errors when there is no Marker file found.
    /// Other errors might pop up later, but this struct is a thin wrapper around a path.
    fn parse(exp_run_dir: &Path) -> Result<Self::Item> {
        let dir = exp_run_dir.to_path_buf();
        if !dir.join(MARKER_RUN).is_file() {
            return Err(Error::LoadingError(format!(
                "run marker file missing in {}",
                dir.display()
            )));
        }

        Ok(Self { dir })
    }
}

// ========================== Tests ==========================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::ExperimentRun;
    use crate::harness::env::Environment;
    use crate::test_fixtures::{env_a1, env_foo};
    use rstest::{fixture, rstest};
    use tracing_test::traced_test;

    use tempfile::TempDir;

    struct RunWithHandles {
        /// handle to keep temp dir w/ run alive
        #[allow(dead_code)]
        tempdir: TempDir,
        source: ExperimentSource,
        run: ExperimentRun,
    }

    #[fixture]
    fn run_with_handles(env_foo: Environment) -> RunWithHandles {
        let tempdir = tempfile::TempDir::new().unwrap();

        let src_dst = tempdir.path().join("source_dir");
        let source = ExperimentSource::create_skeleton(src_dst).unwrap();

        let run_dst = tempdir.path().join("run_dir");
        let run = ExperimentRun::create(&source, &env_foo, run_dst.clone()).unwrap();
        RunWithHandles {
            tempdir,
            run,
            source,
        }
    }

    #[rstest]
    #[traced_test]
    fn create_run_template_copied(run_with_handles: RunWithHandles, env_foo: Environment) {
        // prepare template
        let template_dir = run_with_handles.source.template();
        std::fs::write(template_dir.join("file"), "").unwrap();
        let subdir = template_dir.join("subdir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(&subdir.join("file"), "").unwrap();

        // create run
        let new_dir = tempfile::TempDir::new().unwrap();
        let dst = new_dir.path().join("run_dir");
        let run = ExperimentRun::create(&run_with_handles.source, &env_foo, dst.clone()).unwrap();
        let dir = run.location();

        assert_eq!(dir.canonicalize().unwrap(), dst.canonicalize().unwrap());

        assert!(dir.is_dir());
        assert!(dir.join(MARKER_RUN).is_file());
        assert!(dir.join("run.sh").is_file());
        assert!(dir.join("file").is_file());
        assert!(dir.join("subdir").join("file").is_file());
        assert!(dir.join("exomat_source").is_symlink());

        let env_from_file = Environment::try_from(&dir.join("environment.env")).unwrap();
        assert_eq!(env_from_file, env_foo);
    }

    #[rstest]
    #[traced_test]
    fn create_run_source_linked(run_with_handles: RunWithHandles, env_foo: Environment) {
        let new_dir = tempfile::TempDir::new().unwrap();
        let dst = new_dir.path().join("run_dir");

        let run = ExperimentRun::create(&run_with_handles.source, &env_foo, dst.clone()).unwrap();
        let dir = run.location();

        assert_eq!(dir.canonicalize().unwrap(), dst.canonicalize().unwrap());

        let source_symlink = dir.join(RUN_SOURCE_SYMLINK_NAME);
        assert!(source_symlink.is_symlink());
        let source_symlink_target = std::fs::read_link(&source_symlink).unwrap();
        assert!(source_symlink_target.is_relative());

        // note: canonicalize() also follows symlinks
        let source_symlink_target_full = source_symlink.canonicalize().unwrap();
        let expected_source_dir_full = run_with_handles.source.location().canonicalize().unwrap();

        assert_eq!(expected_source_dir_full, source_symlink_target_full);
    }

    #[rstest]
    fn create_run_dst_exists(run_with_handles: RunWithHandles, env_foo: Environment) {
        let new_dir = tempfile::TempDir::new().unwrap();

        assert!(ExperimentRun::create(
            &run_with_handles.source,
            &env_foo,
            new_dir.path().to_path_buf()
        )
        .is_err());
    }

    #[rstest]
    fn run_env_set(mut run_with_handles: RunWithHandles) {
        let run = &mut run_with_handles.run;
        let debug_script = r#"#!/usr/bin/env bash
set -euo pipefail

echo $FOO > out_foo_val
"#;
        std::fs::write(run.location().join(RUN_RUN_FILE), debug_script).unwrap();

        run.execute().unwrap();

        let foo_file = run.location().join("out_foo_val");
        assert!(foo_file.is_file());
        assert_eq!("foo", std::fs::read_to_string(foo_file).unwrap().trim());
    }

    #[rstest]
    #[case(0)]
    #[case(1)]
    #[case(42)]
    fn run_status_dumped(mut run_with_handles: RunWithHandles, #[case] return_code: u32) {
        let run = &mut run_with_handles.run;
        let debug_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

exit {return_code}
"#
        );
        std::fs::write(run.location().join(RUN_RUN_FILE), debug_script).unwrap();

        let exp_started_file = run.location().join(RUN_OUT_EXOMAT_START_TIMESTAMP);
        let exp_exit_status_file = run.location().join(RUN_OUT_EXOMAT_RUN_EXIT_CODE);

        assert!(!exp_started_file.is_file());
        assert!(!exp_exit_status_file.is_file());

        // always Ok(), even on non-zero exit
        run.execute().unwrap();

        assert!(exp_started_file.is_file());
        assert!(exp_exit_status_file.is_file());

        let return_code_s = std::fs::read_to_string(&exp_exit_status_file).unwrap();
        assert_eq!(return_code_s, format!("{return_code}"));
    }

    #[rstest]
    #[case(RunStatus::NotStarted)]
    #[case(RunStatus::Success)]
    #[case(RunStatus::ExitNonZero(42))]
    fn read_status_from_disk(
        mut run_with_handles: RunWithHandles,
        #[case] given_status: RunStatus,
    ) {
        let run = &mut run_with_handles.run;
        let exp_started_file = run.location().join(RUN_OUT_EXOMAT_START_TIMESTAMP);
        let exp_exit_status_file = run.location().join(RUN_OUT_EXOMAT_RUN_EXIT_CODE);

        match &given_status {
            RunStatus::NotStarted => {} //nop
            RunStatus::Success | RunStatus::ExitNonZero(_) => {
                std::fs::write(exp_started_file, "0").unwrap()
            }
        }

        let maybe_exit_code_to_write = match &given_status {
            &RunStatus::ExitNonZero(code) => Some(code),
            &RunStatus::NotStarted => None,
            &RunStatus::Success => Some(0),
        };
        if let Some(exit_code_to_write) = maybe_exit_code_to_write {
            std::fs::write(exp_exit_status_file, format!("{exit_code_to_write}")).unwrap();
        }

        // check status corectly read from disk
        let parsed_run = ExperimentRun::parse(run.location()).unwrap();
        assert_eq!(given_status, parsed_run.status().unwrap());
    }

    #[rstest]
    fn read_out_files(
        mut run_with_handles: RunWithHandles,
        #[values(true, false)] out_a: bool,
        #[values(true, false)] out_b: bool,
    ) {
        let run = &mut run_with_handles.run;
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
    #[case(env_a1())]
    fn read_env(mut run_with_handles: RunWithHandles, #[case] env: Environment) {
        let run = &mut run_with_handles.run;
        env.to_file(run.location().join(RUN_ENV_FILE).as_path())
            .unwrap();

        assert_eq!(env, run.env().unwrap());
    }

    #[rstest]
    fn read_run_name(mut run_with_handles: RunWithHandles) {
        let run = &mut run_with_handles.run;
        assert_eq!(run.run_name(), run.location().file_name().unwrap());
    }

    #[rstest]
    fn run_execute_source_symlink_exposed_to_var(mut run_with_handles: RunWithHandles) {
        let run = &mut run_with_handles.run;
        let debug_script = format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

test -n "${{{ENV_NAME_EXP_SOURCE_DIR}}}"
echo "${{{ENV_NAME_EXP_SOURCE_DIR}}}" > out_path
"#
        );
        std::fs::write(run.location().join(RUN_RUN_FILE), debug_script).unwrap();

        assert!(run.execute().is_ok());

        let source_path_from_env_string =
            std::fs::read_to_string(run.location().join("out_path")).unwrap();
        let source_path_from_env_path = PathBuf::from(source_path_from_env_string.trim());
        assert!(source_path_from_env_path.is_absolute());

        assert_eq!(
            source_path_from_env_path.canonicalize().unwrap(),
            run_with_handles.source.location().canonicalize().unwrap()
        );
    }
}
