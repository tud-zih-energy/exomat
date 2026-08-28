use std::path::PathBuf;

use crate::Result;
use exomat::experiment::{ExperimentSource, FileReader};

pub fn main(
    experiment: PathBuf,
    trial: bool,
    output: Option<PathBuf>,
    repetitions: u64,
) -> Result<()> {
    let mut src = ExperimentSource::parse(&experiment)?;
    src.set_exomat_envs(exomat::harness::env::ExomatEnvironment::new(
        &experiment,
        repetitions,
    ));

    match trial {
        false => exomat::harness::run::experiment(&src, output, false),
        true => exomat::harness::run::trial(&src),
    }
}
