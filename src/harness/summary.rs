//! harness summary subcommand

use crate::experiment::{ExperimentSource, FileReader};
use crate::helper::errors::{Error, Result};

use chrono::Local;
use log::{debug, trace};
use std::path::PathBuf;
use std::time::Duration;

/// entrypoint for summary binary
///
/// Summarizes the Experiment Source at `source`.
///
/// ## Parameters
/// - `estimate`:
///     - `None` -> No estimated runtime is printed
///     - `Some(None)` -> Estimate is printed, based on a few possible durations per run
///     - `Some(x)` -> Estimate is printed based on x seconds per run
/// - `full`:
///     - `true` -> Additional information about the Experiment Source is printed
///     - `false` -> No additional information printed
///
/// ## Errors and Panics
/// - returns an `EnvError` if `source` cannot be parsed by ExperimentSource
/// - returns a `SummaryError` if `estimate.is_none()` and `full` is false
/// - returns a `SummaryError` if the estimated runtime could not be calculated
/// - panics if the name of the Experiment could not be read from `source`
pub fn main(source: &PathBuf, estimate_s: Option<Option<u64>>, full: bool) -> Result<()> {
    trace!("Parsing experiment Source...");
    let source = ExperimentSource::parse(&source)?;
    let exp_name = source.name().unwrap();

    // check that correct arguments were passed
    if estimate_s.is_none() && !full {
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
    if let Some(per_run) = estimate_s {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::FileWriter;

    use tempfile::TempDir;

    #[test]
    fn summary_no_source() {
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();

        assert!(main(&tmpdir, None, false).is_err())
    }

    #[test]
    fn summary_e2e() {
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();

        let mut source = ExperimentSource::new();
        source.persist(&tmpdir).unwrap();

        assert!(main(&tmpdir, None, false).is_err());

        assert!(main(&tmpdir, Some(None), false).is_ok());
        assert!(main(&tmpdir, Some(Some(0)), false).is_ok());
        assert!(main(&tmpdir, Some(Some(1)), false).is_ok());

        assert!(main(&tmpdir, None, true).is_ok());
        assert!(main(&tmpdir, Some(None), true).is_ok());
        assert!(main(&tmpdir, Some(Some(0)), true).is_ok());
        assert!(main(&tmpdir, Some(Some(1)), true).is_ok());
    }
}
