use indicatif::MultiProgress;
use std::path::PathBuf;

use crate::Result;
use exomat::experiment::{ExperimentSource, FileReader};

pub fn main(
    experiment: PathBuf,
    trial: bool,
    output: Option<PathBuf>,
    repetitions: u64,
    log_handler: MultiProgress,
) -> Result<()> {
    let mut src = ExperimentSource::parse(&experiment)?;
    src.set_exomat_envs(exomat::harness::env::ExomatEnvironment::new(
        &experiment,
        repetitions,
    ));

    match trial {
        false => exomat::harness::run::experiment(&src, output, log_handler, false),
        true => exomat::harness::run::trial(&src, log_handler),
    }
}
