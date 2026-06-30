use crate::experiment::{ExperimentSource, FileReader};
use crate::helper::errors::{Error, Result};

use log::{debug, trace};
use std::path::PathBuf;
use std::time::Duration;

pub fn main(source: PathBuf, estimate: Option<u64>, full: bool) -> Result<()> {
    trace!("Parsing experiment Source...");
    let source = ExperimentSource::parse(&source)?;

    // check that correct arguments were passed
    if estimate.is_some() && full || estimate.is_none() && !full {
        // no arguments or more than one argument given
        return Err(Error::SummaryError {
            experiment: source.name().unwrap(),
            err: String::from("Invalid arguments"),
        });
    };

    if let Some(per_run) = estimate {
        // calculate estimation
        let rep_count = source.repetitions();

        debug!("Calculating estimation with {rep_count} repetitions and {per_run}s per run");
        let estimation = chrono::Duration::from_std(Duration::from_secs(rep_count * per_run))
            .map_err(|e| Error::SummaryError {
                experiment: source.name().unwrap(),
                err: e.to_string(),
            })?;

        // print estimation
        println!(
            "[{}] at {}s/run: {}:{}:{}",
            source.name().unwrap(),
            per_run,
            estimation.num_hours(),
            estimation.num_minutes(),
            estimation.num_seconds(),
        );
    } else if full {
        // print summary
        todo!()
    }

    Ok(())
}
