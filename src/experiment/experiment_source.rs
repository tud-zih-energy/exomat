use log::{info, warn};
use std::{fs::OpenOptions, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};

use crate::experiment::{FileReader, FileWriter};
use crate::harness::env::EnvironmentContainer;
use crate::helper::archivist::{create_harness_dir, create_harness_file};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Container for an Experiment Source
#[derive(Debug, Clone)]
pub struct ExperimentSource {
    run_sh: String,
    envs: EnvironmentContainer,
}

impl ExperimentSource {
    pub fn new() -> Self {
        ExperimentSource {
            run_sh: String::new(),
            envs: EnvironmentContainer::new(),
        }
    }

    pub fn get_envs(&self) -> &EnvironmentContainer {
        &self.envs
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentSource {
    type Item = ExperimentSource;

    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        let run_sh = std::fs::read_to_string(&dir.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE))?;
        let envs = EnvironmentContainer::from_files(&dir.join(SRC_ENV_DIR))?;

        Ok(Self { run_sh, envs })
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
    fn persist(&self, dir: &PathBuf) -> Result<()> {
        create_harness_dir(dir)?;
        create_harness_file(&dir.join(MARKER_SRC))?;

        // create envs if some are given, otherwise just create an empty env file
        create_harness_dir(&dir.join(SRC_ENV_DIR))?;
        if self.envs.environment_count() == 0 {
            create_harness_file(&dir.join(SRC_ENV_DIR).join(SRC_ENV_FILE))?;
        } else {
            self.envs.serialize_environments(&dir.join(SRC_ENV_DIR))?;
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
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use faccess::PathExt;
    use std::fs::read_to_string;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn persist_source_default() {
        let template = read_to_string(PathBuf::from("src/harness/run.sh.template")).unwrap();

        // create base tempdir, to act as parent
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path();
        std::env::set_current_dir(&tmpdir).unwrap();

        // create experiment source dir (relative to current dir)
        let src_path = tmpdir.join("FooSource");
        let src = ExperimentSource::new();

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
    fn persist_source_custom() {}
}

// TODO: move tests from skeleton
