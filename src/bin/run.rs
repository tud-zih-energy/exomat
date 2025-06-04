use exomat::helper::{errors::Error, fs_names::*};
use indicatif::MultiProgress;
use std::path::PathBuf;

use crate::Result;

pub fn main(
    experiment: PathBuf,
    trial: Option<PathBuf>,
    output: Option<PathBuf>,
    repetitions: u64,
    log_handler: MultiProgress,
) -> Result<()> {
    let experiment = experiment.canonicalize()?;
    if experiment.display().to_string() == "." {
        return Err(Error::HarnessRunError {
            experiment: file_name_string(&experiment),
            err: "Cannot start experiment run from the experiment source folder.".to_string(),
        });
    }

    if let Some(env) = trial {
        exomat::run_trial(&experiment, env, log_handler)
    } else {
        let output = match output {
            Some(x) => Ok(x),
            None => exomat::harness::skeleton::generate_build_series_filepath(&experiment),
        }?;

        exomat::run_experiment(&experiment, repetitions, output, log_handler)
    }
}
