//! Functions that touch the filesystem.

use chrono::{DateTime, Local};
use fs_extra::{
    dir::{copy as copy_dir, CopyOptions as DCopyOptions},
    file::{copy as copy_file, CopyOptions as FCopyOptions},
};
use log::debug;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};

use crate::helper::errors::{Error, Result};

/// Generates and validates the path to an output file based on user input.
///
/// If a path is given, it is used as-is, otherwise a default time-based name is generated.
///
/// ## Errors
/// - if generated or given path already exists
pub fn generate_filepath(
    given_output: Option<PathBuf>,
    format: &str,
    timepoint: &DateTime<Local>,
) -> std::result::Result<PathBuf, String> {
    let output =
        given_output.unwrap_or_else(|| PathBuf::from(format!("{}", timepoint.format(format))));

    // check that file does not exist yet
    match output.exists() {
        true => Err(format!("{} already exists.", output.display())),
        false => Ok(output),
    }
}

/// Try to create all dirs in given path.
///
/// Equivalent to `mkdir -p`.
///
/// If successful, returns the path to the newly created directory. Else retruns
/// a `HarnessCreateError`.
pub fn create_harness_dir(directory: &PathBuf) -> Result<PathBuf> {
    create_dir_all(directory).map_err(|e| Error::HarnessCreateError {
        entry: directory.display().to_string(),
        reason: e.to_string(),
    })?;

    Ok(directory.to_owned())
}

/// Creates a new, empty file with all it's parents at `file`.
///
/// If successful, returns the path to the newly created file. Else retruns
/// a `HarnessCreateError`.
pub fn create_harness_file(file: &PathBuf) -> Result<PathBuf> {
    File::create_new(file).map_err(|e| Error::HarnessCreateError {
        entry: file.display().to_string(),
        reason: e.to_string(),
    })?;

    Ok(file.to_owned())
}

/// Copies the content of one file to another.
///
/// Both files have to exist prior to calling this function.
///
/// Retruns a `HarnessCreateError` if something went wrong.
pub fn copy_harness_file(from: &PathBuf, to: &PathBuf) -> Result<()> {
    match copy_file(from, to, &FCopyOptions::new()) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::HarnessCreateError {
            entry: to.display().to_string(),
            reason: e.to_string(),
        }),
    }
}

/// Copies the content of one direcory into another, without creating a new folder
/// in the destination directory.
///
/// Both directories have to exist prior to calling this function.
///
/// Retruns a `HarnessCreateError` if something went wrong.
pub fn copy_harness_dir(from: &PathBuf, to: &PathBuf) -> Result<()> {
    match copy_dir(from, to, &DCopyOptions::new().content_only(true)) {
        Ok(_) => Ok(()),
        Err(e) => Err(Error::HarnessCreateError {
            entry: to.display().to_string(),
            reason: e.to_string(),
        }),
    }
}

/// find the parent dir containing the given marker file, starting at pwd
///
/// # Errors
/// - pwd could not be determined
pub fn find_marker_pwd(marker_name: &str) -> Result<PathBuf> {
    debug!("searching for marker {marker_name} from pwd");
    find_marker(&std::env::current_dir()?, marker_name)
}

/// find the parent dir that contains the given marker name
///
/// Works with nested files.
/// Uses PWD if location is not given.
pub fn find_marker(location: &Path, marker_name: &str) -> Result<PathBuf> {
    if !location.is_absolute() {
        let location = location.to_path_buf().canonicalize()?;
        return find_marker(&location, marker_name);
    }

    if !location.is_dir() {
        return Err(Error::FindMarkerError(
            "location does not exist/is not dir".to_string(),
        ));
    }

    if location.join(marker_name).is_file() {
        debug!("found marker {marker_name} in {}", location.display());
        return Ok(location.to_path_buf());
    }

    // try to check in parent
    match location.parent() {
        Some(parent) => find_marker(parent, marker_name),
        None => Err(Error::FindMarkerError(
            "traversed up to fs root, no marker found; maybe go somewhere else using cd?".to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;
    use rusty_fork::rusty_fork_test;
    use tempfile::TempDir;

    use super::*;

    const TEST_FMT: &str = "test_fmt-%Y-%m-%d-%H-%M-%S";

    #[test]
    fn default_name_generation() {
        let now = Local::now();

        let generated_path = generate_filepath(None, TEST_FMT, &now).unwrap();
        let expected_path = format!("{}", now.format(TEST_FMT));

        assert_eq!(expected_path, generated_path.to_str().unwrap());
        assert!(!generated_path.exists());
    }

    rusty_fork_test! {
        #[test]
        fn uses_given() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(tmpdir).unwrap();

            let now = Local::now();

            let generated_path = generate_filepath(Some(PathBuf::from("foobar")), TEST_FMT, &now).unwrap();
            assert_eq!("foobar", generated_path.to_str().unwrap());
        }

        #[test]
        fn file_already_exists() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            let now = Local::now();
            std::env::set_current_dir(tmpdir).unwrap();

            // passes: does not exist yet
            let path_default = generate_filepath(None, TEST_FMT, &now).unwrap();
            let path_given = generate_filepath(Some(PathBuf::from("foo")), TEST_FMT, &now).unwrap();

            std::fs::File::create(&path_default).unwrap();
            std::fs::File::create(&path_given).unwrap();

            // must fail after existance
            assert!(generate_filepath(None, TEST_FMT, &now).is_err());
            assert!(generate_filepath(Some(PathBuf::from("foo")), TEST_FMT, &now).is_err());

            // but other names still work
            assert!(generate_filepath(Some(PathBuf::from("bar")), TEST_FMT, &now).is_ok());
        }

        #[test]
        fn find_marker_nested() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(tmpdir).unwrap();

            std::fs::create_dir_all("base/foo/bar/baz").unwrap();
            std::fs::create_dir_all("base/foo/bar/foo").unwrap();
            std::fs::create_dir_all("base/foo/a").unwrap();
            std::fs::create_dir_all("base/bar/baz").unwrap();
            std::fs::write("base/foo/.my_marker", "").unwrap();

            let base_abs = PathBuf::from("base/foo").canonicalize().unwrap();

            let dir_finds_base = |path: &str| {
                match find_marker(&PathBuf::from(path), ".my_marker") {
                    Err(_) => false,
                    Ok(path) if base_abs == path => true,
                    Ok(_) => panic!("unknown path found, wtf"),
                }
            };

            assert!(!dir_finds_base("base"));
            assert!(dir_finds_base("base/foo"));
            assert!(dir_finds_base("base/foo/bar"));
            assert!(dir_finds_base("base/foo/bar/baz"));
            assert!(dir_finds_base("base/foo/bar/foo"));
            assert!(dir_finds_base("base/foo/a"));
            assert!(!dir_finds_base("base/bar/baz"));
            assert!(!dir_finds_base("base/bar"));
        }
    }
}
