use indicatif::MultiProgress;
use std::path::PathBuf;

use crate::Result;

pub fn main(
    experiment: PathBuf,
    trial: bool,
    output: Option<PathBuf>,
    repetitions: u64,
    log_handler: MultiProgress,
) -> Result<()> {
    let experiment = experiment.canonicalize()?;

    match trial {
        false => {
            let output = match output {
                Some(x) => Ok(x),
                None => exomat::harness::skeleton::generate_build_series_filepath(&experiment),
            };

            match output {
                Ok(output) => exomat::harness::run::experiment(
                    &experiment,
                    repetitions,
                    output,
                    log_handler,
                    false,
                ),
                Err(err) => Err(err),
            }
        }
        true => exomat::harness::run::trial(&experiment, log_handler),
    }
}
