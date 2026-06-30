use crate::experiment::{ExperimentSource, FileReader};
use crate::helper::errors::{Error, Result};

use log::trace;

pub fn main(estimate: Option<u64>, full: bool) -> Result<()> {
    trace!("Parsing experiment Source...");
    let source = ExperimentSource::parse(&std::env::current_dir()?)?;

    // check that correct arguments were passed
    if estimate.is_some() && full || estimate.is_none() && !full {
        // no arguments or more than one argument given
        return Err(Error::SummaryError {
            experiment: source.name().unwrap(),
            err: String::from("Invalid arguments"),
        });
    };

    if estimate.is_some() {
        // calculate estimation
        todo!()
    } else if full {
        // print summary
        todo!()
    }

    Ok(())
}
