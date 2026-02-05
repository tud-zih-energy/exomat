use super::fs_names::*;
use std::fs::OpenOptions;
use std::{io::Write, path::PathBuf};

use crate::harness::env::ExomatEnvironment;
use crate::harness::skeleton::{build_series_directory, create_source_directory};

/// helper to create a `run.sh` file in an experiment source directory.
///
/// When executed, it will write the content of `${out_env}` to stdout and in `out_file`
pub fn filled_run_in(exp_src: &PathBuf, out_env: &str) {
    let run_sh_path = exp_src.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE);
    let content = format!("echo ${out_env}\necho ${out_env} >> out_file");

    let mut run_sh = OpenOptions::new().append(true).open(&run_sh_path).unwrap();
    run_sh.write(content.as_bytes()).unwrap();
}

/// helper that reads a file at `[location]/[SERIES_RUNS_DIR]/[log_name]`
pub fn read_log(location: PathBuf, log_name: &str) -> String {
    std::fs::read_to_string(location.join(SERIES_RUNS_DIR).join(log_name)).unwrap()
}

/// helper to create a file at `location` with content `content`
pub fn create_env_at(location: &PathBuf, content: &str) {
    let mut env = std::fs::File::create(location).unwrap();
    env.write_all(content.as_bytes()).unwrap();
}

/// generates an experiment source and an experiment series dir in `base`
///
/// returns (source_path, series_path, default_env_path, exomat_envs)
pub fn skeleton_src_series_in(
    base: &PathBuf,
    src_name: &str,
    series_name: &str,
) -> (PathBuf, PathBuf, PathBuf, ExomatEnvironment) {
    let source = base.join(src_name);
    let series = base.join(series_name);

    create_source_directory(&source).unwrap();
    build_series_directory(&source, &series).unwrap();

    let default_env = source.join(SRC_ENV_DIR).join(SRC_ENV_FILE);
    let exomat_env = ExomatEnvironment::new(&source, 1);

    (source, series, default_env, exomat_env)
}
/// Checks if the given string contains either one or the other
pub fn contains_either(string: &String, one: &str, other: &str) -> bool {
    string.contains(one) || string.contains(other)
}

pub fn create_out_file(series_dir: &PathBuf, name: &str, content: &str) {
    let outfile = series_dir
        .join(SERIES_RUNS_DIR)
        .join(TEST_RUN_REP_DIR0)
        .join(name);

    std::fs::File::create(&outfile).unwrap();
    std::fs::write(outfile, content).unwrap();
}
