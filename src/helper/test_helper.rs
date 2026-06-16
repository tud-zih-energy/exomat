use super::fs_names::*;
use std::fs::OpenOptions;
use std::{io::Write, path::PathBuf};

use crate::experiment::{ExperimentSeries, ExperimentSource, FileWriter};
use crate::harness::env::ExomatEnvironment;

/// helper to create a `run.sh` file in an experiment source directory.
///
/// When executed, it will write the content of `${out_env}` to stdout and in `out_file`
pub fn place_filled_run_in(exp_src: &PathBuf, out_env: &str) {
    let run_sh_path = exp_src.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE);
    let content = format!("echo ${out_env}\necho ${out_env} >> out_file");

    let mut run_sh = OpenOptions::new().append(true).open(&run_sh_path).unwrap();
    run_sh.write(content.as_bytes()).unwrap();
}

/// helper that reads a file at `[location]/[SERIES_RUNS_DIR]/[log_name]`
pub fn read_log(location: PathBuf, log_name: &str) -> String {
    std::fs::read_to_string(location.join(SERIES_RUNS_DIR).join(log_name))
        .unwrap()
        .trim()
        .to_string()
}

/// helper to create a file at `location` with content `content`
pub fn create_file_at(location: &PathBuf, content: &str) {
    let mut env = std::fs::File::create(location).unwrap();
    env.write_all(content.as_bytes()).unwrap();
}

/// generates an experiment source and an experiment series dir in `base`
///
/// Only the source gets serialized.
pub fn populate_src_with_series(
    base: &PathBuf,
    src_name: &str,
    series_name: &str,
) -> (ExperimentSource, ExperimentSeries) {
    let source = base.join(src_name);
    let series = base.join(series_name);

    let mut src = ExperimentSource::new();
    src.set_exomat_envs(ExomatEnvironment::new(&source, 1));
    src.persist(&source).unwrap();

    let mut ser = ExperimentSeries::from_source(&src).unwrap();
    ser.set_location(series);

    (src, ser)
}
/// Checks if the given string contains either one or the other
pub fn contains_either(string: &String, one: &str, other: &str) -> bool {
    string.contains(one) || string.contains(other)
}

/// Creates a file called `name` in `series_dir/[SERIES_RUNS_DIR]/rep_name/` with the content `content`
///
/// If `rep_name` is `None`, [TEST_RUN_REP_DIR0] is used.
pub fn create_out_file(series_dir: &PathBuf, rep_name: Option<&str>, name: &str, content: &str) {
    let outfile = series_dir
        .join(SERIES_RUNS_DIR)
        .join(rep_name.unwrap_or(TEST_RUN_REP_DIR0))
        .join(name);

    std::fs::File::create(&outfile).unwrap();

    if !content.is_empty() {
        std::fs::write(outfile, content).unwrap();
    }
}
