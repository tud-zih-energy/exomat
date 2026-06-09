//! harness make-table command

use log::info;
use std::path::PathBuf;

use crate::helper::errors::Result;
use crate::helper::fs_names::*;

use crate::experiment::{ExperimentSeries, FileReader};

/// Entrypoint for table binary
///
/// Filters output (files) from every run repetition in the pwd.
/// Looks through every `series_dir/runs/run_*` directory and accumulates the content of
/// every `out_*` file into one csv file.
///
/// ## Example
/// ```bash
/// exp_series
/// \-> runs
///     |-> run_0_rep0
///     |   |-> out_foo # content: "42"
///     |   \-> out_bar # content: "true"
///     \-> run_0_rep1
///         |-> out_foo # content: "300"
///         \-> out_bar # content: "false"
/// ```
/// results in `exp_series.csv` with:
/// ```notest
/// foo,bar
/// 42, true
/// 300,false
/// ```
pub fn main() -> Result<()> {
    let series_dir = crate::find_marker_pwd(MARKER_SERIES)?;

    // collect all output from every run in series_dir
    let reader = ExperimentSeries::parse(&series_dir)?;

    let keys = reader.keys();
    info!("Collected output for {} keys", keys.len());
    info!("Found keys: {:?}", keys);

    // output file will be "series_dir/[series_dir].csv"
    let mut out_file = PathBuf::from(
        series_dir
            .file_name()
            .expect("Could not read experiment series name"),
    );
    out_file.set_extension("csv");

    // serialize data and write to file
    reader.to_csv(&series_dir.join(out_file))
}
