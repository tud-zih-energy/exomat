//! harness skeleton subcommand

use chrono::Local;
use log::{debug, info};
use std::{
    fs::OpenOptions,
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

use crate::duplicate_log_to_file;
use crate::harness::env::{exomat_environment::append_exomat_envs, ExomatEnvironment};
use crate::helper::archivist::{
    copy_harness_dir, copy_harness_file, create_harness_dir, create_harness_file,
};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

/// Creates an empty experiment source folder.
///
/// The folder will be created with the following content:
/// ```notest
/// exp_source_dir
///   |-> .exomat_source
///   |-> SRC_TEMPLATE_DIR/
///   | \-> SRC_RUN_FILE [content: src/harness/run.sh.template]
///   \-> SRC_ENV_DIR/
///     \-> SRC_ENV_FILE [EMPTY]
/// ```
///
/// ## Errors
/// - Returns an `HarnessCreateError` if any entry of the list above could not be created.
///
/// ## Example
/// ```
/// use exomat::harness::skeleton::create_source_directory;
/// use exomat::helper::fs_names::*;
///
/// use std::path::PathBuf;
/// use tempfile::TempDir;
/// use faccess::PathExt;
///
/// // read run.sh template before changing pwd
/// assert!(PathBuf::from("src/harness/run.sh.template").is_file());
/// let template = std::fs::read_to_string(PathBuf::from("src/harness/run.sh.template")).unwrap();
///
/// // create base tempdir, to act as parent
/// let tmpdir = TempDir::new().unwrap();
/// let tmpdir = tmpdir.path();
/// std::env::set_current_dir(&tmpdir).unwrap();
///
/// // create experiment source dir (relative to current dir)
/// let exp_source = PathBuf::from("FooSource");
/// create_source_directory(&exp_source).unwrap();
///
/// assert!(&tmpdir.join("FooSource").is_dir());
/// assert!(exp_source.join(SRC_ENV_DIR).is_dir());
/// assert!(exp_source.join(SRC_ENV_DIR).join(SRC_ENV_FILE).is_file());
/// assert!(exp_source.join(SRC_TEMPLATE_DIR).is_dir());
/// assert!(exp_source.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE).is_file());
///
/// // new run.sh contains template, is executable
/// let run_file = PathBuf::from(&exp_source.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE));
/// let run = std::fs::read_to_string(&run_file).unwrap();
/// assert_eq!(run, template);
/// assert!(&run_file.executable());
/// ```
pub fn create_source_directory(exp_src_dir: &PathBuf) -> Result<()> {
    create_harness_dir(exp_src_dir)?;
    create_harness_file(&exp_src_dir.join(MARKER_SRC))?;

    create_harness_dir(&exp_src_dir.join(SRC_ENV_DIR))?;
    create_harness_file(&exp_src_dir.join(SRC_ENV_DIR).join(SRC_ENV_FILE))?;
    create_harness_dir(&exp_src_dir.join(SRC_TEMPLATE_DIR))?;

    let run_file_path = &exp_src_dir.join(SRC_TEMPLATE_DIR).join(SRC_RUN_FILE);

    // create default run.sh as executable
    let mut run_file = OpenOptions::new()
        .mode(0o775)
        .write(true)
        .create_new(true)
        .open(run_file_path)
        .map_err(|e| Error::HarnessCreateError {
            entry: run_file_path.to_str().unwrap().to_string(),
            reason: e.to_string(),
        })?;

    // write default content to run.sh
    let template_runfile_bytes = include_bytes!("run.sh.template");
    run_file.write_all(template_runfile_bytes)?;

    info!("Experiment harness created under {}", exp_src_dir.display());
    Ok(())
}

/// Creates and populates a new experiment series directory.
///
/// The new directory will have this structure:
/// ```notest
/// SERIES_DIR
///   |-> .exomat_series
///   |-> .src/
///   | |-> .exomat_source_cp  [replaces .exomat_source]
///   | \-> [copy of experiment source directory, read-only]
///   \-> runs/
///     |-> stdout.log [EMPTY]
///     |-> stderr.log [EMPTY]
///     \-> exomat.log [EMPTY]
/// ```
/// > Note: This example assumes values for [SERIES_SRC_DIR], [SERIES_RUNS_DIR], [SERIES_STDERR_LOG],
/// > [SERIES_STDOUT_LOG], [SERIES_EXOMAT_LOG]. The names in the actual file structure might
/// > differ, depending on the values of them.
///
/// This function will not overwrite an existing series directory.
///
/// Once the log files have been created any log output by exomat will be duplicated
/// to them.
///
/// ## Errors and Panics
/// - Returns a `HarnessCreateError` if there is an experiment series directory
///   called `series_name` in the same directory
/// - Panics if `exp_source` could not be read
pub fn build_series_directory(exp_source: &PathBuf, series_dir: &Path) -> Result<()> {
    debug!(
        "attempting to build series directory from {}",
        exp_source.display()
    );
    debug!("checking if is dir");
    if !exp_source.is_dir() {
        return Err(Error::HarnessRunError {
            experiment: exp_source.to_string_lossy().to_string(),
            err: "is not directory".to_string(),
        });
    }

    debug!("checking if source dir marker exists");
    if !exp_source.join(MARKER_SRC).is_file() {
        return Err(Error::HarnessRunError {
            experiment: exp_source.to_string_lossy().to_string(),
            err: "is not an experiment source directory".to_string(),
        });
    }

    // check if series dir is valid
    fn is_child_dir_of_of(maybe_child: &Path, parent: &Path) -> Result<bool> {
        let parent = parent.canonicalize()?;

        Ok(maybe_child
            .ancestors()
            .any(|ancestor| match ancestor.canonicalize() {
                Ok(ancestor) => ancestor == parent,
                Err(_) => false, // dir does not exist -> is certainly not parent
            }))
    }

    debug!("checking if creating series inside of experiment (would be forbidden)");
    if is_child_dir_of_of(series_dir, exp_source)? {
        // log full paths to debug, but let error be handled (i.e. reported as error) outside
        debug!("refusing to build series dir inside of experiment dir, experiment dir: {}, to-be-created series dir: {}",
               exp_source.display(),
               series_dir.display());
        return Err(Error::HarnessRunError {
            experiment: exp_source.to_string_lossy().to_string(),
            err: "can not generate output inside of experiment dir".to_string(),
        });
    }

    let src = create_harness_dir(&series_dir.join(SERIES_SRC_DIR))?;
    let runs = create_harness_dir(&series_dir.join(SERIES_RUNS_DIR))?;

    let _ = create_harness_file(&series_dir.join(MARKER_SERIES))?;
    let _ = create_harness_file(&runs.join(SERIES_STDOUT_LOG))?;
    let _ = create_harness_file(&runs.join(SERIES_STDERR_LOG))?;
    let exomat_log = create_harness_file(&runs.join(SERIES_EXOMAT_LOG))?;

    duplicate_log_to_file(&exomat_log);

    // copy exp_source/template to src and replace marker
    copy_harness_dir(exp_source, &src)?;
    std::fs::remove_file(src.join(MARKER_SRC))?;
    create_harness_file(&src.join(MARKER_SRC_CP))?;

    info!(
        "Created new experiment series dir at {}",
        series_dir.display()
    );

    Ok(())
}

/// Build the filepath to a new series directory.
///
/// Generates either a trial run location, or a new name in the PWD.
///
/// The name will be derived from the experiment name and the current date and time.
pub fn generate_build_series_filepath(exp_source: &Path) -> Result<PathBuf> {
    let format = format!("{}-%Y-%m-%d-%H-%M-%S", file_name_string(exp_source));
    let dirname = PathBuf::from(Local::now().format(&format).to_string());
    Ok(std::env::current_dir()?
        .canonicalize()?
        .join(&dirname)
        .to_path_buf())
}

/// Creates a ready-to-use experiment run folder for **one interation** with **one environment**
/// of an experiment.
///
/// ### Note: `env_file` is used to deduce the `{env}` part of the new experiment run directory name.
/// ###       `exomat_environment` is used to get the `{it}` part.
///
/// The new directory will be created in the given `series_folder` under [SERIES_RUNS_DIR]`/run_[env]_rep[repetition]`.
/// This will result in the following structure:
/// ```notest
/// series_folder
///   |-> ...
///   \-> runs/
///     |-> ...
///     \-> run_{env}_rep{it}/
///       |-> .exomat_run
///       |-> RUN_RUN_FILE     (copy of SRC_RUN_FILE)
///       \-> RUN_ENV_FILE     (copy of env_file)
/// ```
///
/// If no Errors occured, the path to the created experiment run folder will be returned.
///
/// ## Errors and Panics
/// - Returns a `HarnessCreateError` if there is no [SERIES_RUNS_DIR] found inside `series_folder`
/// - Returns a `HarnessCreateError` if any file or directory could not be created or copied
/// - Panics if `it_format_length` is 0
pub fn build_run_directory(
    series_folder: &Path,
    env_file: &PathBuf,
    exomat_environment: &ExomatEnvironment,
    it_format_length: usize,
) -> Result<PathBuf> {
    assert!(it_format_length > 0, "repetition format cannot be 0");

    // unwrap here, because this should never fail and if it does it's your fault
    let env_name = &env_file.file_stem().unwrap().to_str().unwrap();

    let run = format!(
        "run_{}_rep{:0length$}",
        env_name,
        exomat_environment.repetition,
        length = it_format_length,
    );

    // get path to runs/, return error if it does not exist
    let runs_dir = match series_folder.join(SERIES_RUNS_DIR).is_dir() {
        true => series_folder.join(SERIES_RUNS_DIR),
        false => {
            return Err(Error::HarnessCreateError {
                entry: run,
                reason: format!(
                    "{} dir does not exist in {}",
                    SERIES_RUNS_DIR,
                    series_folder.display()
                ),
            })
        }
    };

    let run = create_harness_dir(&runs_dir.join(run))?;
    create_harness_file(&run.join(MARKER_RUN))?;

    let copy_run = series_folder.join(SERIES_SRC_DIR).join(SRC_TEMPLATE_DIR);

    // copy ruh.sh and [env].env to runs_dir
    let run_to_cp = copy_run.join(SRC_RUN_FILE);
    copy_harness_file(&run_to_cp, &run.join(RUN_RUN_FILE))?;
    copy_harness_file(&env_file, &run.join(RUN_ENV_FILE))?;

    // write any exomat variables to file that need to be written
    append_exomat_envs(&run.join(RUN_ENV_FILE), exomat_environment)?;

    Ok(run)
}

/// entrypoint for skeleton binary
pub fn main(exp_src_dir: &PathBuf) -> Result<()> {
    create_source_directory(exp_src_dir)?;

    println!();
    println!("next steps:");
    println!("1. add variables with:");
    println!("   exomat env --add COUNT 1 2 3");
    println!("2. adjust script in template/run.sh");
    println!("3. execute experiment with:");
    println!("   exomat run {}", exp_src_dir.display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use rusty_fork::rusty_fork_test;
    use tempfile::TempDir;

    use super::*;
    use crate::harness::env::{
        exomat_environment::append_exomat_envs, Environment, ExomatEnvironment,
    };

    #[test]
    fn test_create_source_multiple_times() {
        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();

        assert!(create_source_directory(&tmpdir).is_ok());
        assert!(matches!(
            create_source_directory(&tmpdir),
            Err(Error::HarnessCreateError {
                entry: _,
                reason: _
            })
        ));
    }

    rusty_fork_test! {
        #[test]
        fn test_create_source_missing_parents() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();

            let with_parents = PathBuf::from_str("foo/bar").unwrap();
            assert!(create_source_directory(&with_parents).is_ok());
            assert!(PathBuf::from_str("foo").unwrap().exists());
            assert!(PathBuf::from_str("foo/bar").unwrap().exists());

            // template is ONLY in foo/bar
            assert!(PathBuf::from_str("foo/bar/envs").unwrap().exists());
            assert!(!PathBuf::from_str("foo/envs").unwrap().exists());
        }

        #[test]
        fn build_run_directory_simple() {
            use crate::helper::fs_names::*;

            use faccess::PathExt;

            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path();
            std::env::set_current_dir(&tmpdir).unwrap();

            // create an experiment source, and an experiment series
            let exp_source = tmpdir.join("FooSource");
            let exp_series = tmpdir.join("FooSeries");
            create_source_directory(&exp_source).unwrap();
            build_series_directory(&exp_source, &exp_series).unwrap();

            // extract an env file to create run directory with and add exomat envs
            let default_env = exp_source.join(SRC_ENV_DIR).join(SRC_ENV_FILE);
            let exomat_env = ExomatEnvironment::new(&exp_source.to_path_buf(), 1);
            append_exomat_envs(&exp_source.join(SRC_ENV_DIR).join(SRC_ENV_FILE), &exomat_env).unwrap();

            // create run dir (based on exp_series, environment from default_env,
            // one repetition, formatrepetitionn without leading zeros)
            let run_dir = build_run_directory(&exp_series, &default_env, &exomat_env, 1).unwrap();
            assert_eq!(tmpdir.join(&run_dir).file_name().unwrap(), "run_0_rep1");

            assert!(tmpdir.join(&run_dir).is_dir());
            assert!(run_dir.join(RUN_ENV_FILE).is_file());
            assert!(run_dir.join(RUN_RUN_FILE).is_file());
            assert!(run_dir.join(RUN_RUN_FILE).executable());

            // check that repetition number is an env
            let envs = Environment::from_file(&run_dir.join(RUN_ENV_FILE)).unwrap();
            assert_eq!(envs.get_env_val("REPETITION"), Some(&String::from("1")));

            // it_format_length changes the name of each experiment run directory:
            let run_dir = build_run_directory(&exp_series, &default_env, &exomat_env, 3).unwrap();
            assert_eq!(tmpdir.join(&run_dir).file_name().unwrap(), "run_0_rep001");
        }

        #[test]
        fn test_internal_envs_not_in_files(){
            // set up source/series/run dir
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path();
            std::env::set_current_dir(&tmpdir).unwrap();

            let exp_source = tmpdir.join("FooSource");
            let exp_series = tmpdir.join("FooSeries");
            create_source_directory(&exp_source).unwrap();
            build_series_directory(&exp_source, &exp_series).unwrap();

            let default_env = exp_source.join(SRC_ENV_DIR).join(SRC_ENV_FILE);
            let mut env = Environment::from_file(&default_env).unwrap();
            env.add_env(String::from("FOO"), String::from("BAR"));
            env.to_file(&default_env).unwrap();

            let exomat_envs = ExomatEnvironment::new(&PathBuf::from("/"), 42); // content does not matter

            let run_dir = build_run_directory(&exp_series, &default_env, &exomat_envs, 1).unwrap();

            // check contents of env files
            let src_env = Environment::from_file(&default_env).unwrap();
            let run_env = Environment::from_file(&run_dir.join(RUN_ENV_FILE)).unwrap();

            // exomat variable, never serialized
            assert!(!src_env.contains_env_var("EXP_SRC_DIR"));
            assert!(!run_env.contains_env_var("EXP_SRC_DIR"));

            // exomat variable, serialized
            assert!(!src_env.contains_env_var("REPETITION"));
            assert!(run_env.contains_env_var("REPETITION"));

            // user variable, always serialized
            assert!(src_env.contains_env_var("FOO"));
            assert!(run_env.contains_env_var("FOO"));
        }

        #[test]
        fn build_series_dir_simple() {
            use crate::helper::fs_names::*;

            // create base tempdir, to act as parent
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path();
            std::env::set_current_dir(&tmpdir).unwrap();

            //create experiment source dir
            let exp_source = tmpdir.join("FooSource");
            let exp_series = tmpdir.join("foo");
            create_source_directory(&exp_source).unwrap();

            // create series dir (next to exp_source, named "foo", is not a trial run)
            build_series_directory(&exp_source, &exp_series).unwrap();

            assert!(tmpdir.join("foo").is_dir());
            assert!(exp_series.join(SERIES_SRC_DIR).is_dir());
            assert!(exp_series.join(SERIES_RUNS_DIR).is_dir());

            assert!(exp_series.join(SERIES_RUNS_DIR).join(SERIES_EXOMAT_LOG).is_file());
            assert!(exp_series.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG).is_file());
            assert!(exp_series.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG).is_file());

            // content of experiment source have been copied to exp_series/src
            // .exomat_source changed to .exomat_source_cp
        }
    }
}
