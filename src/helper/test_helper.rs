use super::fs_names::*;
use std::fs::OpenOptions;
use std::{io::Write, path::PathBuf};

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
