use std::path::PathBuf;

use crate::experiment::FileReader;
use crate::harness::env::EnvironmentContainer;
use crate::helper::{errors::Result, fs_names::*};

/// Container for an Experiment Source
pub struct ExperimentSource {
    name: String,
    run_sh: String,
    envs: EnvironmentContainer,
}

// ========================== Reader ==========================
impl FileReader for ExperimentSource {
    type Item = ExperimentSource;

    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        let name = file_name_string(&dir);
        let run_sh = std::fs::read_to_string(&dir.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE))?;
        let envs = EnvironmentContainer::from_files(&dir.join(SRC_ENV_DIR))?;

        Ok(Self { name, run_sh, envs })
    }
}
