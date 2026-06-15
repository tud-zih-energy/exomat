use indicatif::MultiProgress;
use std::path::PathBuf;

use crate::Result;
use exomat::experiment::{ExperimentSeries, ExperimentSource, FileReader};

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
        false => {
            let output = match output {
                Some(x) => Ok(x),
                None => ExperimentSeries::generate_series_filepath(&experiment),
            };

            match output {
                Ok(output) => exomat::harness::run::experiment(&src, output, log_handler, false),
                Err(err) => Err(err),
            }
        }
        true => exomat::harness::run::trial(&src, log_handler),
    }
}
