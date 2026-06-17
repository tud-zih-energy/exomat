use log::{info, warn};
use std::{
    collections::HashMap, fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf,
};

use crate::experiment::{FileReader, FileWriter};
use crate::harness::env::{
    get_existing_environments_by_fname, Environment, EnvironmentContainer, EnvironmentLocationList,
    ExomatEnvironment,
};
use crate::helper::archivist::{create_harness_dir, create_harness_file};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Container for an Experiment Source
#[derive(Debug, Clone)]
pub struct ExperimentSource {
    run_sh: String,
    envs: EnvironmentLocationList,
    exomat_envs: ExomatEnvironment,
}

impl ExperimentSource {
    pub fn new() -> Self {
        ExperimentSource {
            run_sh: include_str!("../harness/run.sh.template").to_string(),
            envs: HashMap::new(),
            exomat_envs: ExomatEnvironment::new(&PathBuf::new(), 1),
        }
    }

    pub fn from(
        run_sh: String,
        envs: EnvironmentLocationList,
        exomat_envs: ExomatEnvironment,
    ) -> Self {
        Self {
            run_sh,
            envs,
            exomat_envs,
        }
    }

    pub fn to_trial_source(&self) -> Result<Self> {
        if self.location().display().to_string() == "." {
            return Err(Error::HarnessRunError {
                experiment: self.name()?,
                err: "Cannot start experiment run from the experiment source folder.".to_string(),
            });
        };

        let trial_env: EnvironmentLocationList = match self.envs.is_empty() {
            true => HashMap::from([(PathBuf::from(SRC_ENV_FILE), Environment::new())]),
            false => HashMap::from([self
                .envs
                .clone()
                .into_iter()
                .next()
                .expect("Cannot access Environment")]),
        };

        Ok(Self {
            run_sh: self.run_sh.clone(),
            envs: trial_env,
            exomat_envs: ExomatEnvironment {
                exp_src_dir: self.location().to_path_buf(),
                repetition: 1,
            },
        })
    }

    pub fn get_envs(&self) -> &EnvironmentLocationList {
        &self.envs
    }

    pub fn name(&self) -> Result<String> {
        if self.exomat_envs.exp_src_dir == PathBuf::new() {
            warn!("Run cannot determine it's source.");
            Err(Error::Empty(
                "EXP_SRC_DIR not set in Experiment Source".to_string(),
            ))
        } else {
            Ok(file_name_string(&self.exomat_envs.exp_src_dir))
        }
    }

    pub fn exomat_envs(&self) -> &ExomatEnvironment {
        &self.exomat_envs
    }

    pub fn repetitions(&self) -> &u64 {
        &self.exomat_envs.repetition
    }

    pub fn location(&self) -> &PathBuf {
        &self.exomat_envs.exp_src_dir
    }

    pub fn run_script(&self) -> &str {
        &self.run_sh
    }

    pub fn set_run_script(&mut self, script: String) {
        self.run_sh = script;
    }

    pub fn set_envs(&mut self, envs: EnvironmentLocationList) -> Result<()> {
        if let Some(invalid_env) = envs
            .keys()
            .find(|env_file_name| env_file_name.extension().unwrap() != "env")
        {
            return Err(Error::EnvError {
                reason: format!("Invalid env file name at {}", invalid_env.display()),
            });
        }

        self.envs = envs;
        Ok(())
    }

    pub fn set_exomat_envs(&mut self, exomat_envs: ExomatEnvironment) {
        self.exomat_envs = exomat_envs;
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentSource {
    type Item = ExperimentSource;

    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        let exomat_envs = ExomatEnvironment::new(
            &dir.to_path_buf()
                .canonicalize()
                .expect("Could not resole Source path"),
            0,
        );
        let run_sh = std::fs::read_to_string(&dir.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE))?;
        let envs = get_existing_environments_by_fname(&dir.join(SRC_ENV_DIR))?;

        Ok(Self {
            run_sh,
            envs,
            exomat_envs,
        })
    }
}

// ========================== Writer ==========================
impl FileWriter for ExperimentSource {
    /// Creates an experiment source folder.
    ///
    /// If nothing is set, the folder will be created with the following defaults:
    /// ```notest
    /// exp_source_dir
    ///   |-> .exomat_source
    ///   |-> SRC_TEMPLATE_DIR/
    ///   | \-> SRC_RUN_FILE [content: src/harness/run.sh.template]
    ///   \-> SRC_ENV_DIR/
    ///     \-> SRC_ENV_FILE [EMPTY]
    /// ```
    ///
    /// ## Errors
    /// - Returns an `HarnessCreateError` if any entry of the list above could not be created.
    fn persist(&mut self, dir: &PathBuf) -> Result<()> {
        create_harness_dir(dir)?;
        create_harness_file(&dir.join(MARKER_SRC))?;

        // create envs if some are given, otherwise just create an empty env file
        create_harness_dir(&dir.join(SRC_ENV_DIR))?;
        if self.envs.len() == 0 {
            create_harness_file(&dir.join(SRC_ENV_DIR).join(SRC_ENV_FILE))?;
        } else {
            let envs = EnvironmentContainer::from_env_list(
                self.envs
                    .clone()
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect::<Vec<Environment>>(),
            );
            envs.serialize_environments(&dir.join(SRC_ENV_DIR))?;
        }

        // create run.sh as executable
        create_harness_dir(&dir.join(SRC_TEMPLATE_DIR))?;
        let run_file_path = &dir.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE);

        let mut run_file = OpenOptions::new()
            .mode(0o775)
            .write(true)
            .create_new(true)
            .open(run_file_path)
            .map_err(|e| Error::HarnessCreateError {
                entry: run_file_path.to_str().unwrap().to_string(),
                reason: e.to_string(),
            })?;

        // write content to run.sh
        let run_sh_bytes = if self.run_sh.is_empty() {
            warn!("Tried to serialize empty run.sh; Used default template instead.");
            include_bytes!("../harness/run.sh.template")
        } else {
            self.run_sh.as_bytes()
        };

        run_file.write_all(run_sh_bytes)?;

        info!("Experiment harness created under {}", dir.display());

        self.exomat_envs.exp_src_dir = dir.to_path_buf();
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use faccess::PathExt;
    use rusty_fork::rusty_fork_test;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_create_source_multiple_times() {
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();

        let mut src = ExperimentSource::new();
        assert!(src.persist(&tmpdir).is_ok());
        assert!(src.persist(&tmpdir).is_err());
    }

    rusty_fork_test! {
        #[test]
        fn persist_source_default() {
            let template = read_to_string(PathBuf::from("src/harness/run.sh.template")).unwrap();

            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path();
            std::env::set_current_dir(&tmpdir).unwrap();

            // create experiment source dir (relative to current dir)
            let src_path = tmpdir.join("FooSource");
            let mut src = ExperimentSource::new();

            src.persist(&src_path).unwrap();

            assert!(&tmpdir.join("FooSource").is_dir());
            assert!(src_path.join(SRC_ENV_DIR).is_dir());
            assert!(src_path.join(SRC_ENV_DIR).join(SRC_ENV_FILE).is_file());
            assert!(src_path.join(SRC_TEMPLATE_DIR).is_dir());

            let run_file = PathBuf::from(&src_path.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE));

            // new run.sh contains template, is executable
            assert!(run_file.is_file());
            let run = read_to_string(&run_file).unwrap();

            assert_eq!(run, template);
            assert!(&run_file.executable());
        }

        #[test]
        fn persist_source_custom() {
            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path();
            std::env::set_current_dir(&tmpdir).unwrap();

            let custom_script = "!#/bin/bash\necho Something";

            // create experiment source dir (relative to current dir)
            let src_path = tmpdir.join("FooSource");
            let mut src = ExperimentSource::new();

            src.set_run_script(custom_script.to_string());
            src.persist(&src_path).unwrap();

            assert!(&tmpdir.join("FooSource").is_dir());
            assert!(src_path.join(SRC_ENV_DIR).is_dir());
            assert!(src_path.join(SRC_ENV_DIR).join(SRC_ENV_FILE).is_file());
            assert!(src_path.join(SRC_TEMPLATE_DIR).is_dir());

            let run_file = PathBuf::from(&src_path.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE));

            // new run.sh contains template, is executable
            assert!(run_file.is_file());
            let run = read_to_string(&run_file).unwrap();

            assert_eq!(run, custom_script);
            assert!(&run_file.executable());
        }

        #[test]
        fn test_create_source_missing_parents() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();

            let with_parents = PathBuf::from("foo/bar");
            assert!(!PathBuf::from("foo").exists());
            assert!(!PathBuf::from("foo/bar").exists());

            let mut src = ExperimentSource::new();
            assert!(src.persist(&with_parents).is_ok());

            assert!(PathBuf::from("foo").exists());
            assert!(PathBuf::from("foo/bar").exists());

            // template is ONLY in foo/bar
            assert!(PathBuf::from("foo/bar/envs").exists());
            assert!(!PathBuf::from("foo/envs").exists());
        }
    }
}
