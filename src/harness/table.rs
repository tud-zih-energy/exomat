//! harness make-table command

use log::info;
use std::path::PathBuf;

use crate::helper::errors::Result;
use crate::helper::fs_names::*;

use crate::experiment::{CsvWriter, ExperimentSeries, FileReader};

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
pub fn main(output: Option<PathBuf>) -> Result<()> {
    let series_dir = crate::find_marker_pwd(MARKER_SERIES)?;

    // collect all output from every run in series_dir
    let reader = ExperimentSeries::parse(&series_dir)?;

    let keys = [reader.env_keys(), reader.output_keys()].concat();
    info!("Collected output for {} keys", keys.len());
    info!("Found keys: {:?}", keys);

    // output file will be "series_dir/[series_dir].csv"
    let out_file = output.unwrap_or_else(|| {
        let mut f = PathBuf::from(
            series_dir
                .file_name()
                .expect("Could not read experiment series name"),
        );
        f.set_extension("csv");
        f
    });

    // serialize data and write to file
    reader.to_csv(&series_dir.join(out_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_fixtures::filled_series_run_na;
    use rusty_fork::rusty_fork_test;
    use tempfile::NamedTempFile;

    rusty_fork_test! {
        #[test]
        fn output_default() {
            let filled_series_run_na = filled_series_run_na();
            std::env::set_current_dir(filled_series_run_na.path()).unwrap();
            main(None).unwrap();
        }

        #[test]
        fn output_set_file() {
            let output_file_a = NamedTempFile::new().unwrap();
            let output_file_b = NamedTempFile::new().unwrap();

            let filled_series_run_na = filled_series_run_na();
            std::env::set_current_dir(filled_series_run_na.path()).unwrap();
            main(Some(PathBuf::from(output_file_a.path()))).unwrap();
            main(Some(PathBuf::from(output_file_b.path()))).unwrap();

            let content_file_a = std::fs::read_to_string(output_file_a).unwrap();
            let content_file_b = std::fs::read_to_string(output_file_b).unwrap();

            assert_ne!("", content_file_a);
            assert_ne!("", content_file_b);
            assert_eq!(content_file_a, content_file_b);
        }
    }
}
