use crate::experiment::{ExperimentSource, FileReader};
use crate::helper::errors::{Error, Result};

use chrono::Local;
use log::{debug, trace};
use std::path::PathBuf;
use std::time::Duration;

pub fn main(source: PathBuf, estimate: Option<Option<u64>>, full: bool) -> Result<()> {
    trace!("Parsing experiment Source...");
    let source = ExperimentSource::parse(&source)?;
    let exp_name = source.name().unwrap();

    // check that correct arguments were passed
    if estimate.is_none() && !full {
        return Err(Error::SummaryError {
            experiment: exp_name.clone(),
            err: String::from("Invalid arguments"),
        });
    };

    // print summary
    if full {
        println!("{source}");
    }

    // calculate estimation
    if let Some(per_run) = estimate {
        let env_count = source.envs().len() as u64;

        let per_run = if let Some(custom_estimate) = per_run {
            vec![custom_estimate]
        } else {
            vec![1, 10, 60]
        };

        println!("[{exp_name}] estimated runtime per repetition");
        for duration in per_run {
            debug!("calculating estimation for {env_count} environment(s), {duration}s per run");
            let estimation = chrono::Duration::from_std(Duration::from_secs(env_count * duration))
                .map_err(|e| Error::SummaryError {
                    experiment: exp_name.clone(),
                    err: e.to_string(),
                })?;

            debug!("calculating ETA");
            let eta = Local::now() + estimation;

            // print estimation
            println!(
                "{}s/run: {:02}:{:02}:{:02} (done {})",
                duration,
                estimation.num_hours(),
                estimation.num_minutes() % 60,
                estimation.num_seconds() % 60,
                eta.format("%H:%M").to_string()
            );
        }
    }
    Ok(())
}
