use crate::experiment::{ExperimentSource, FileReader};
use crate::helper::errors::{Error, Result};

use chrono::Local;
use log::{debug, trace};
use std::path::PathBuf;
use std::time::Duration;

pub fn main(source: PathBuf, estimate: Option<Option<u64>>, full: bool) -> Result<()> {
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

    // calculate estimation
    if let Some(per_run) = estimate {
        let rep_count = source.repetitions();

        let per_run = if let Some(custom_estimate) = per_run {
            vec![custom_estimate]
        } else {
            vec![1, 10, 60]
        };

        for duration in per_run {
            debug!("calculating estimation with {rep_count} repetition(s) and {duration}s per run");
            let estimation = chrono::Duration::from_std(Duration::from_secs(rep_count * duration))
                .map_err(|e| Error::SummaryError {
                    experiment: source.name().unwrap(),
                    err: e.to_string(),
                })?;

            debug!("calculation ETA");
            let eta = Local::now() + estimation;

            // print estimation
            println!(
                "[{}] at {}s/run: {:02}:{:02}:{:02} (done {})",
                source.name().unwrap(),
                duration,
                estimation.num_hours(),
                estimation.num_minutes() % 60,
                estimation.num_seconds() % 60,
                eta.format("%H:%M").to_string()
            );
        }
    } else if full {
        // print summary
        todo!()
    }

    Ok(())
}
