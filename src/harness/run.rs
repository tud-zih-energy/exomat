//! harness run subcommand

use chrono::Local;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{error, info, trace, warn};
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::{
    fs::OpenOptions,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use strip_ansi::strip_ansi;

use super::env::{fetch_environment_files, Environment, ExomatEnvironment};
use super::skeleton::{build_run_directory, build_series_directory};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Creates an experiment series/run directory for the given `experiment`.
/// Then executes the `run.sh` file for this experiment and dumps the output in
/// the log files.
///
/// The new experiment series directory will either be called `[experiment]-YYYY-MM-DD-HH-MM-SS`
/// or whatever is defined in `output`.
///
/// Requires a directory called `[experiment]` to be present in the current location.
///
/// Wrapper around `build_series_directory` and `execute_exp_repetitions`.
pub fn experiment(
    experiment: &PathBuf,
    repetitions: u64,
    output: PathBuf,
    log_progress_handler: MultiProgress,
    is_trial: bool,
) -> Result<()> {
    build_series_directory(experiment, &output)?;
    execute_exp_repetitions(
        experiment,
        &output,
        repetitions,
        log_progress_handler,
        is_trial,
    )
}

/// Creates an experiment series/run directory for the given `experiment`.
/// Then executes the `run.sh` file for this experiment once and collects any
/// output/errors/results.
///
/// The new experiment series directory will be created as a tempdir.
pub fn trial(experiment: &PathBuf, log_progress_handler: MultiProgress) -> Result<()> {
    let exp_name = file_name_string(&experiment.canonicalize().unwrap());

    if experiment.display().to_string() == "." {
        return Err(Error::HarnessRunError {
            experiment: exp_name,
            err: "Cannot start experiment run from the experiment source folder.".to_string(),
        });
    }

    let format = &Local::now()
        .format("exomat_trial-%Y-%m-%d-%H-%M-%S")
        .to_string();

    let trial_dir_path = std::env::temp_dir().join(format);

    crate::disable_console_log();

    // run experiment once
    let res = self::experiment(
        experiment,
        1,
        trial_dir_path.clone(),
        log_progress_handler,
        true,
    );

    // flush exomat log
    spdlog::default_logger().flush();

    // gather results
    let stdout =
        std::fs::read_to_string(trial_dir_path.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG))?;
    let stderr =
        std::fs::read_to_string(trial_dir_path.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG))?;
    let exomat =
        std::fs::read_to_string(trial_dir_path.join(SERIES_RUNS_DIR).join(SERIES_EXOMAT_LOG))?;
    let out_files = collect_output(&trial_dir_path)?;

    let eval_res = create_report(&exp_name, &res, &stdout, &stderr, &out_files, &exomat);
    print!("{eval_res}");

    res
}

/// Filters all "out_$NAME" files from the given experiment series directory. Then creates
/// a map with each out_$NAME becomming a key and the content of this file becomming the associated value.
///
/// Uses `crate::harness::table::collect_output()`, but does not include any environment variables.
///
/// ## Errors and Panics
/// - Panics if there is more than one run/repetition in the series directory
///
/// The content of `out_$NAME` files is not validated or checked in any way, if you put
/// weird content in them, you will get weird output.
fn collect_output(dir: &PathBuf) -> Result<HashMap<String, String>> {
    let mut output = HashMap::<String, String>::new();
    let out_files = crate::harness::table::collect_output(&dir)?; //TODO change

    // there should only be one iteration, since it's a trial
    assert!(out_files.values().all(|content| content.len() == 1));

    for (name, content) in out_files.iter() {
        if ExomatEnvironment::RESERVED_ENV_VARS.contains(&name.as_str()) {
            continue;
        }

        output.insert(
            format!("out_{name}"),
            content
                .first()
                .expect("Could not extract out_* content")
                .to_owned(),
        );
    }

    Ok(output)
}

/// Runs the experiment defined in `exp_source_dir` `repetitions` times for each
/// environment.
///
/// This will create a new experiment run folder inside `exp_series_dir`.
///
/// This functions assumes that `build_series_directory` has been called before it.
/// Otherwise it will fail, because the files it expects to be there are not.
fn execute_exp_repetitions(
    exp_source_dir: &Path,
    exp_series_dir: &Path,
    repetitions: u64,
    log_progress_handler: MultiProgress,
    is_trial: bool,
) -> Result<()> {
    let length = repetitions.to_string().len();
    let envs = fetch_environment_files(&exp_source_dir.join(SRC_ENV_DIR)).ok_or_else(|| {
        Error::HarnessRunError {
            experiment: exp_source_dir.display().to_string(),
            err: format!(
                "No environments found in {}",
                exp_source_dir.join(SRC_ENV_DIR).display()
            ),
        }
    })?;

    let prog_bar = ProgressBar::new(repetitions * envs.len() as u64);
    prog_bar.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] [{bar:.green}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("#>-"),
    );

    // protect progress bar from log interferance
    let prog_bar = log_progress_handler.add(prog_bar);
    prog_bar.tick(); // show on 0th repetition

    info!("Starting experiment runs for {}", exp_source_dir.display());

    let running_order: Vec<(&PathBuf, u64)> = shuffle_experiments(&envs, &repetitions);
    for (environment, rep) in running_order {
        let exomat_envs = ExomatEnvironment::new(&exp_source_dir.to_path_buf(), rep);
        trace!("exomat envs are: {:?}", exomat_envs.to_environment_full());

        let run_folder = build_run_directory(exp_series_dir, &environment, &exomat_envs, length)?;
        trace!("Using envs: {:?}", Environment::from_file(&environment)?);

        run_experiment(
            &file_name_string(exp_source_dir),
            &run_folder,
            &exomat_envs.to_environment_full(),
        )?;

        // update progress
        prog_bar.inc(1);

        // stop after one run if this is a trial
        if is_trial {
            break;
        }
    }

    prog_bar.finish();
    Ok(())
}

/// Compiles a list of all repetition for each environment, then suffles said list.
///
/// The shuffled list is then sorted by repetition, so that all n-repetitions run
/// before all n+1-repetitions.
fn shuffle_experiments<'a>(
    environments: &'a Vec<PathBuf>,
    repetition_count: &'a u64,
) -> Vec<(&'a PathBuf, u64)> {
    let mut running_order = vec![];

    for env in environments {
        for rep in 0..*repetition_count {
            running_order.push((env, rep));
        }
    }

    running_order.shuffle(&mut rand::rng());
    running_order.sort_by(|a, b| (a.1).cmp(&b.1));

    return running_order;
}

/// Executes [RUN_RUN_FILE] script found in `run_folder`.
///
/// 1. read envs from `run_folder/RUN_ENV_FILE`
/// 2. add `exomat_envs` (overwrites envs with the same name)
/// 3. run `run_folder/RUN_RUN_FILE` with these envs
/// 4. log run results
///     - Appends any stderr/stdout output into their respective log file in the
///       parent series directory of `run_folder`.
///     - Exomat output will **not** automatically be duplicated to the log file
///       by calling this function.
///
/// ## Errors and Panics
/// - Returns a `HarnessRunrror` if [RUN_RUN_FILE] could not be executed
/// - panics if there is no [RUN_RUN_FILE] in `run_folder`
/// - panics if there is no [RUN_ENV_FILE] in `run_folder`
fn run_experiment(exp_name: &str, run_folder: &Path, exomat_envs: &Environment) -> Result<()> {
    assert!(
        run_folder.join(RUN_RUN_FILE).is_file(),
        "Missing run.sh in experiment run directory"
    );

    assert!(
        run_folder.join(RUN_ENV_FILE).is_file(),
        "Missing environment.env in experiment run directory"
    );

    // This loads all existing process environment variables, plus the variables
    // found in `run_folder/RUN_ENV_FILE`.
    // Then adds the reserved exomat variables, without persisting them in a file.
    let mut envs = Environment::from_file_with_load(&run_folder.join(RUN_ENV_FILE))?;
    envs.extend_envs(exomat_envs);

    let out_log = OpenOptions::new()
        .append(true)
        .open(run_folder.parent().unwrap().join(SERIES_STDOUT_LOG))?;

    let err_log = OpenOptions::new()
        .append(true)
        .open(run_folder.parent().unwrap().join(SERIES_STDERR_LOG))?;

    trace!(
        "{exp_name}: Starting execution of {}",
        run_folder.file_stem().unwrap().to_str().unwrap()
    );

    let run_folder_absolute = &run_folder.canonicalize().unwrap();

    // execute command with envs and collect any output in child
    let run = Command::new(run_folder_absolute.join(RUN_RUN_FILE))
        .stderr(Stdio::from(err_log))
        .stdout(Stdio::from(out_log))
        .envs(envs.to_env_map())
        .current_dir(run_folder_absolute)
        .output()
        .map_err(|e| Error::HarnessRunError {
            experiment: exp_name.to_string(),
            err: e.to_string(),
        })?;

    trace!("{exp_name}: Finished run {}", run_folder.display());

    // open file again, but in read-only mode
    log_run_result(
        run_folder.file_stem().unwrap().to_str().unwrap(),
        run.status,
        &mut OpenOptions::new()
            .read(true)
            .open(run_folder.parent().unwrap().join(SERIES_STDERR_LOG))?,
    )
}

/// Produce log output based on exit_status and err_log content.
///
/// - exit_status:
///    - **success**  : log info
///    - **failed**   : log error (don't evaluate err_log after)
/// - err_log:
///    - **empty**    : log info
///    - **not empty**: log warning
///
/// ## Errors
/// - Returns a HarnessRunError if `exit_status` shows a failure
fn log_run_result(
    run_name: &str,
    exit_status: std::process::ExitStatus,
    err_log: &mut std::fs::File,
) -> Result<()> {
    // read stderr
    let mut stderr = String::new();
    err_log.read_to_string(&mut stderr)?;

    if exit_status.success() {
        info!("{run_name} finished successfully with {exit_status}");

        if stderr.is_empty() {
            info!("{run_name} did not produce stderr output");
        } else {
            warn!("{run_name} produced stderr output");
        }
    } else {
        error!("{run_name} finished with non-zero {exit_status}");

        // fail fast in case of unsuccessful run
        return Err(Error::HarnessRunError {
            experiment: run_name.to_string(),
            err: String::from(strip_ansi(&stderr).trim()),
        });
    }

    Ok(())
}

/// Creates a ready-to-print String based on the given parameters.
///
/// ## Example
/// Given the values:
/// - `exp_name = Foo`
/// - `run = Ok(_)`
/// - `stdout = "normal output"`
/// - `stderr = ""`
/// - `out_files = {"out_foo": "some content". "out_bar": "42"}`
/// - `exomat = "[info] ..."`
///
/// ```bash
/// [Foo] exomat:
/// [info] ...
/// ---
/// [Foo] stdout:
/// normal output
/// ---
/// [Foo] stderr:
///
/// ---
/// [Foo] out_foo:
/// some content
///
/// [Foo] out_bar:
/// 42
///
/// ---
/// [Foo] returned:
/// Successful
/// ```
///
/// An extra "\n" will be added to `stdout`, `stderr` and `exomat`.
///
/// If `run = Err(e)`, the last lines will be:
/// ```bash
/// [Foo] returned:
/// Failed (reason: e)
/// ```
fn create_report<T>(
    exp_name: &str,
    run: &Result<T>,
    stdout: &str,
    stderr: &str,
    out_files: &HashMap<String, String>,
    exomat: &str,
) -> String {
    let mut eval_str = String::new();

    // append exomat
    eval_str.push_str(&format!("[{exp_name}] exomat:\n"));
    eval_str.push_str(exomat);
    eval_str.push_str("\n---\n");

    // append stdout
    eval_str.push_str(&format!("[{exp_name}] stdout:\n"));
    eval_str.push_str(stdout);
    eval_str.push_str("\n---\n");

    // append stderr
    eval_str.push_str(&format!("[{exp_name}] stderr:\n"));
    eval_str.push_str(stderr);
    eval_str.push_str("\n---\n");

    // append out file content
    if out_files.is_empty() {
        eval_str.push_str("[{exp_name}] created no output files\n")
    } else {
        for (out_file, content) in out_files.iter() {
            eval_str.push_str(&format!("[{exp_name}] {out_file}:\n{content}\n\n"));
        }
    }
    eval_str.push_str("---\n");

    // append overall success/failure report
    eval_str.push_str(&format!("[{exp_name}] returned:\n"));
    match run {
        Ok(_) => eval_str.push_str("Successful\n"),
        Err(e) => eval_str.push_str(&format!("Failed (reason: {e})\n")),
    }

    eval_str
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use rusty_fork::rusty_fork_test;
    use tempfile::TempDir;

    use super::super::skeleton::{
        build_run_directory, build_series_directory, create_source_directory,
    };
    use super::*;
    use crate::harness::env::ExomatEnvironment;

    rusty_fork_test! {
        #[test]
        fn test_run() {
            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let series_dir_handle = TempDir::new().unwrap();

            // create experiment source and series dir
            let exp_source = TempDir::new_in(tmpdir.path()).unwrap();
            let exp_source = exp_source.path().to_path_buf();
            std::env::set_current_dir(&exp_source).unwrap();
            create_source_directory(&exp_source).unwrap();

            // write something in run.sh
            let mut runsh = OpenOptions::new()
                .append(true)
                .open(exp_source.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE))
                .unwrap();

            writeln!(runsh, "echo $EXP_SRC_DIR").unwrap();

            let series = series_dir_handle.path();
            build_series_directory(&exp_source, series).unwrap();

            let exomat_envs = ExomatEnvironment::new(&exp_source, 0);
            let default_env = series
                .join(SERIES_SRC_DIR)
                .join(SRC_ENV_DIR)
                .join(SRC_ENV_FILE);

            // create run dir and run experiment
            let run = build_run_directory(series, &default_env, &exomat_envs, 1).unwrap();
            run_experiment(&file_name_string(&exp_source), &run, &exomat_envs.to_environment_full()).unwrap();

            let out_log = std::fs::read_to_string(series.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG)).unwrap();
            let err_log = std::fs::read_to_string(series.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG)).unwrap();

            assert!(out_log.contains(&exp_source.canonicalize().unwrap().display().to_string()));
            assert!(err_log.is_empty());
        }

        #[test]
        fn harness_run_e2e() {
            // create ouput dir
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();
            let exp_name = "SomeExperiment";
            let out_name = "ExpOutput";

            // build basic experiment
            crate::harness::skeleton::main(&PathBuf::from(exp_name)).unwrap();

            // Write something to run.sh that uses env var
            let mut run_sh = OpenOptions::new()
                .append(true)
                .open(
                    &tmpdir
                        .join(exp_name)
                        .join(SRC_TEMPLATE_DIR)
                        .join(SRC_RUN_FILE),
                )
                .unwrap();
            run_sh
                .write("echo $FOO\necho $FOO >> out_file".as_bytes()) // write to stdout and in file
                .unwrap();

            // make multiple .env files that set $FOO to different values
            let mut env1 =
                std::fs::File::create(&tmpdir.join(exp_name).join(SRC_ENV_DIR).join("0.env")).unwrap();

            let mut env2 =
                std::fs::File::create(&tmpdir.join(exp_name).join(SRC_ENV_DIR).join("m.env")).unwrap();

            env1.write_all("FOO=BAR".as_bytes()).unwrap();
            env2.write_all("FOO=Z".as_bytes()).unwrap();

            // run experiment and check logs
            experiment(
                &PathBuf::from(exp_name.to_string()),
                1,
                PathBuf::from(out_name),
                MultiProgress::new(), // empty
                false
            )
            .unwrap();

            let stdout_log = std::fs::read_to_string(
                tmpdir
                    .join(out_name)
                    .join(SERIES_RUNS_DIR)
                    .join(SERIES_STDOUT_LOG),
            )
            .unwrap();
            let stderr_log = std::fs::read_to_string(
                tmpdir
                    .join(out_name)
                    .join(SERIES_RUNS_DIR)
                    .join(SERIES_STDERR_LOG),
            )
            .unwrap();

            assert_eq!(stderr_log.lines().count(), 0);
            // two lines for variable (inserted here), two lines for current time (part of run.sh template)
            assert_eq!(dbg!(stdout_log.lines()).count(), 4);
            assert!(stdout_log.contains("Z"));
            assert!(stdout_log.contains("BAR"));

            // take one out_file and check its content
            let output = std::fs::read_to_string(
                tmpdir
                    .join(out_name)
                    .join(SERIES_RUNS_DIR)
                    .join("run_0_rep0/out_file"),
            )
            .unwrap();
            assert_eq!(output.lines().count(), 1);
            assert!(output.contains("BAR"));
        }

        #[test]
        fn trial_e2e() {
            // create ouput dir
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();
            let exp_name = "SomeExperiment";

            // build basic experiment
            crate::harness::skeleton::main(&PathBuf::from(exp_name)).unwrap();

            // Write something to run.sh that uses env var
            let mut run_sh = OpenOptions::new()
                .append(true)
                .open(&tmpdir.join(exp_name).join("template").join("run.sh"))
                .unwrap();
            run_sh
                .write("echo $FOO\necho $FOO >> out_file".as_bytes()) // write to stdout and in file
                .unwrap();

            // make multiple .env files that set $FOO to different values
            let mut env1 =
                std::fs::File::create(&tmpdir.join(exp_name).join("envs").join("0.env")).unwrap();

            let mut env2 =
                std::fs::File::create(&tmpdir.join(exp_name).join("envs").join("m.env")).unwrap();

            env1.write_all("FOO=BAR".as_bytes()).unwrap();
            env2.write_all("FOO=Z".as_bytes()).unwrap();

            // no error
            trial(&PathBuf::from(exp_name), MultiProgress::new()).unwrap();
        }
    }

    #[test]
    fn collect_out_no_files() {
        // collect on dir without out_* files
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();

        std::fs::create_dir_all(&series_dir).unwrap();
        std::fs::File::create(series_dir.join("random_file")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn collect_out_empty() {
        // collect on dir with out_* file, without content
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_dir = series_dir.join("runs/run_0_rep0");

        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::File::create(run_dir.join("out_empty")).unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res.get("out_empty").unwrap(), "");
    }

    #[test]
    fn collect_out_working() {
        // collect on dir with out_* files, with content
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_dir = series_dir.join("runs/run_0_rep0");

        std::fs::create_dir_all(&run_dir).unwrap();
        std::fs::write(run_dir.join("out_full"), "foo bar").unwrap();

        let res = collect_output(&series_dir).unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res.get("out_full").unwrap(), "foo bar");
    }

    #[test]
    #[should_panic]
    fn collect_out_too_many_runs() {
        // collect on dir with out_* files from multiple runs
        let series_dir = TempDir::new().unwrap();
        let series_dir = series_dir.path().to_path_buf();
        let run_dir_1 = series_dir.join("runs/run_0_rep0");
        let run_dir_2 = series_dir.join("runs/run_7_rep4");

        std::fs::create_dir_all(&run_dir_1).unwrap();
        std::fs::write(run_dir_1.join("out_1"), "foo bar").unwrap();

        std::fs::create_dir_all(&run_dir_2).unwrap();
        std::fs::write(run_dir_2.join("out_1"), "something else").unwrap();

        let _this_panics = collect_output(&series_dir);
    }
}
