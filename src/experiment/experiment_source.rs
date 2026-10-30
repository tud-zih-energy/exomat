use std::path::{Path, PathBuf};
use tracing::debug;

use crate::experiment::FileReader;
use crate::helper::archivist::create_harness_file;
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Representation of an experiment source dir on disk
#[derive(Debug, Clone)]
pub struct ExperimentSource {
    dir: PathBuf,
}

impl ExperimentSource {
    /// Create a skeleton of an experiment source on disk
    pub fn create_skeleton(dir: PathBuf) -> Result<Self> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        debug!("creating dir");
        std::fs::create_dir(&dir)?;

        debug!("dumping run.sh");
        let template_dir = dir.join(SRC_TEMPLATE_DIR);
        std::fs::create_dir(&template_dir)?;

        let mut run_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .mode(0o755)
            .open(&template_dir.join(SRC_RUN_FILE))?;
        run_file.write_all(include_bytes!("../harness/run.sh.template"))?;

        debug!("setting up initial env");
        // note: just create empty file, don't even bother using the wrappers
        let env_dir = dir.join(SRC_ENV_DIR);
        std::fs::create_dir(&env_dir)?;
        create_harness_file(&env_dir.join(SRC_ENV_FILE))?;

        debug!("done, adding marker file");
        create_harness_file(&dir.join(MARKER_SRC))?;

        Ok(Self { dir })
    }

    // ========================= getter ========================================

    /// A reference to the map of Environments with their file names as keys
    ///
    /// Stores the content of `[experiment]/SRC_ENV_DIR/`
    pub fn envs(&self) -> &crate::harness::env::EnvironmentLocationList {
        todo!("remove or refactor")
        // &self.envs
    }

    /// Returns the directory name of the Experiment Source.
    pub fn name(&self) -> Result<String> {
        Ok(self
            .dir
            .file_name()
            .ok_or(Error::Empty(
                "experiment source had empty path, wtf?".to_string(),
            ))?
            .to_string_lossy()
            .to_string())
    }

    /// A reference to the internal variables of this Experiment Source
    pub fn exomat_envs(&self) -> &crate::harness::env::ExomatEnvironment {
        todo!("remove or refactor")
        // &self.exomat_envs
    }

    /// Location of the Experiment Source in the Filesystem
    pub fn location(&self) -> &PathBuf {
        &self.dir
    }

    pub fn template(&self) -> PathBuf {
        self.dir.join(SRC_TEMPLATE_DIR)
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentSource {
    type Item = ExperimentSource;

    /// Create an ExperimentSource based on a directory.
    ///
    /// The following values are set:
    /// - `run_sh`: content of `dir/[SRC_TEMPLATE_DIR]/[SRC_RUN_FILE]`
    /// - `envs`: content of `dir/[SRC_ENV_DIR]`
    /// - `exomat_envs`:
    ///     - `exp_src_dir`: dir (absolute path)
    ///     - `repetition`: 1
    ///
    /// ## Panics
    /// - returns an `IoError` if the run script could not be read
    /// - returns an `EnvError` if Environments could not be parsed
    /// - panics if the absolute path of `dir` cannot be build
    fn parse(dir: &Path) -> Result<Self::Item> {
        let dir = dir.canonicalize()?;
        if !dir.join(MARKER_SRC).is_file() {
            return Err(Error::ReaderError {
                dir: dir.to_string_lossy().to_string(),
                reason: "marker file for source dir is missing".to_string(),
            });
        }

        Ok(Self { dir })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    use std::fs::read_to_string;
    use tempfile::TempDir;
    use tracing_test::traced_test;

    #[test]
    #[traced_test]
    fn skeleton_regular() {
        let work_dir = TempDir::new().unwrap();
        let new_dir = work_dir.path().join("new_dir");
        ExperimentSource::create_skeleton(new_dir.clone()).unwrap();

        assert!(new_dir.join("envs").is_dir());
        assert!(new_dir.join("envs").join("0.env").is_file());
        assert!(new_dir.join(MARKER_SRC).is_file());
        let run_file_path = new_dir.join("template").join("run.sh");
        assert!(run_file_path.is_file());
        let run_file_content = read_to_string(&run_file_path).unwrap();
        assert_eq!(run_file_content, include_str!("../harness/run.sh.template"));
    }

    #[test]
    fn skeleton_target_exists() {
        let existing_dir = TempDir::new().unwrap();
        let error =
            ExperimentSource::create_skeleton(existing_dir.path().to_path_buf()).unwrap_err();
        assert!(error.to_string().contains("exist"));
    }

    #[test]
    fn parse_skeleton() {
        let existing_dir = TempDir::new().unwrap();
        let dir = existing_dir.path().join("my_dir");
        ExperimentSource::create_skeleton(dir.clone()).unwrap();
        let parsed = ExperimentSource::parse(&dir).unwrap();
        assert_eq!(dir, parsed.dir);
    }

    #[test]
    fn parse_no_marker() {
        let existing_dir = TempDir::new().unwrap();
        let dir = existing_dir.path().join("my_dir");
        ExperimentSource::create_skeleton(dir.clone()).unwrap();
        std::fs::remove_file(dir.join(MARKER_SRC)).unwrap();
        let _ = ExperimentSource::parse(&dir).unwrap_err();
    }
}
