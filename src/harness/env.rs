//! harness env subcommand

use itertools::Itertools;
use log::{debug, info, trace};
use regex::Regex;
use std::collections::HashMap;
use std::path::PathBuf;

pub mod environment;
pub mod environment_container;

use crate::helper::archivist::find_marker_pwd;
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::SRC_ENV_DIR;
pub use environment::Environment;
pub use environment_container::EnvironmentContainer;

/// map of all variables with all possible values
///
/// ## Example
/// - `0.env`: FOO=true, BAR=1
/// - `1.env`: FOO=true, BAR=2
/// - `2.env`: FOO=false, BAR=1
/// - `3.env`: FOO=false, BAR=2
///
/// can be encoded in an EnvVarList like this:
/// - `["FOO" = ["true", "false"], "BAR" = ["1", "2"]]`
pub type EnvVarList = HashMap<String, Vec<String>>;

/// Collects paths of all .env files in `from`. Returns `None` if
/// no .env files were found.
///
/// ## Example
/// ```
/// use exomat::harness::env::fetch_env_files;
/// use tempfile::TempDir;
///
/// let env_dir = TempDir::new().unwrap();
/// let env_dir = env_dir.path().to_path_buf();
///
/// // file with .env extension
/// let mock_env_file = tempfile::Builder::new()
///     .suffix(".env")
///     .tempfile_in(&env_dir)
///     .unwrap();
/// let mock_env_file = mock_env_file.path().to_path_buf();
///
/// // file without .env extension
/// let random_file = tempfile::Builder::new().tempfile_in(&env_dir).unwrap();
/// let random_file = random_file.path().to_path_buf();
///
/// let found_files = fetch_env_files(&env_dir).unwrap();
///
/// // recognized only the .env file
/// assert_eq!(found_files.len(), 1);
/// assert!(found_files.contains(&mock_env_file));
/// assert!(!found_files.contains(&random_file));
/// ```
///
/// ## Panics
/// - Panics if `from` could not be read or is not a directory
pub fn fetch_env_files(from: &PathBuf) -> Option<Vec<PathBuf>> {
    assert!(from.is_dir(), "Given dir is not a directory");

    let files = std::fs::read_dir(from)
        .map_err(Error::IoError)
        .unwrap()
        .filter_map(|result| result.ok()) // entry is readable
        .filter(|entry| entry.metadata().unwrap().is_file()) // entry is file
        .filter(|file| file.file_name().to_str().unwrap().ends_with(".env")) // filter .env files
        .map(|env_file| env_file.path()) // turn to path
        .collect::<Vec<PathBuf>>();

    match files.is_empty() {
        true => None,
        false => Some(files),
    }
}

/// Set the $EXP_SRC_DIR env in `src_dir` to the absolute path of`src_dir`
///
/// Will overwrite $EXP_SRC_DIR if it is invalid of missing, otherwise does nothing.
/// Touches all `.env` files if one contains an invalid value.
pub fn validate_src_env(src_dir: &PathBuf) -> Result<()> {
    let exp_src_dir = src_dir
        .canonicalize()
        .expect("could not determine experiment source dir")
        .display()
        .to_string();

    let existing = fetch_env_files(&src_dir.join(SRC_ENV_DIR)).unwrap_or(vec![]);

    // rewrite $EXP_SRC_DIR if it is incorrect in a file
    for env_file in &existing {
        let mut env_content = Environment::from_file(env_file)?;
        let needs_update = match env_content.get_value("EXP_SRC_DIR") {
            Some(val) if val == &exp_src_dir => false,
            _ => true,
        };

        if needs_update {
            env_content.add_variable("EXP_SRC_DIR".to_string(), exp_src_dir.clone());
            env_content.to_file(&env_file)?;
        }
    }

    Ok(())
}

/// Check if a condition is true for any iterator `T`.
///
/// ## Errors
/// - Returns an error message if no iterator satisfies the condition
fn assert_exists<T, F>(iter: T, condition: F) -> std::result::Result<(), String>
where
    T: IntoIterator,
    F: Fn(T::Item) -> bool,
{
    iter.into_iter()
        .any(condition)
        .then_some(())
        .ok_or_else(|| String::from("Item does not exist."))
}

/// Adds all possible combinations of all values in `to_add` to `given`.
///
/// # Example
/// ```ignore
/// use std::collections::HashMap;
/// use exomat::harness::env::try_assemble_all;
///
/// let given = HashMap::from([("1".to_string(), "a".to_string())]);
/// let to_add = HashMap::from([
///     ("2".to_string(), vec!["b".to_string(), "c".to_string()]),
///     ("3".to_string(), vec!["42".to_string(), "43".to_string()])
/// ]);
///
/// let assembled = try_assemble_all(&given, &to_add).unwrap();
/// assert_eq!(assembled.len(), 4);
///
/// // all possible combinations of values that should be formed
/// assert!(assembled.contains(&HashMap::from([
///     ("1".to_string(), "a".to_string()),
///     ("2".to_string(), "b".to_string()),
///     ("3".to_string(), "42".to_string()),
///     ])
/// ));
///
/// assert!(assembled.contains(&HashMap::from([
///     ("1".to_string(), "a".to_string()),
///     ("2".to_string(), "b".to_string()),
///     ("3".to_string(), "43".to_string()),
///     ])
/// ));
///
/// assert!(assembled.contains(&HashMap::from([
///     ("1".to_string(), "a".to_string()),
///     ("2".to_string(), "c".to_string()),
///     ("3".to_string(), "42".to_string()),
///     ])
/// ));
///
/// assert!(assembled.contains(&HashMap::from([
///     ("1".to_string(), "a".to_string()),
///     ("2".to_string(), "c".to_string()),
///     ("3".to_string(), "43".to_string()),
///     ])
/// ));
/// ```
///
/// # Errors
/// - Returns `EnvError` if a key from `to_add` is already in `given`
fn try_assemble_all(given: &Environment, to_add: &EnvVarList) -> Result<Vec<Environment>> {
    // combine all values from to_add
    let mut combinations = EnvironmentContainer::from_env_list(
        to_add
            .values()
            .multi_cartesian_product()
            .collect::<Vec<_>>() // list of all possible value combinations without keys
            .into_iter()
            .map(|val_combos| {
                let pairs = to_add
                    .keys()
                    .cloned()
                    .zip(val_combos.iter().map(|s| s.to_string()))
                    .collect::<Vec<(String, String)>>();
                Environment::from_env_list(pairs)
            })
            .collect(),
    );

    trace!("Adding env combinations: {combinations:?}");

    // add existing variables to each list
    combinations.extend_environemnts(given);
    debug!("Finished assembling environments: {combinations:?}");

    Ok(combinations.to_env_list().to_owned())
}

/// Takes a list of `Vec<Vec<String>>` and turns it into a `HashMap<String, Vec<String>>`.
/// The first element of each `Vec<String>` will be used as a key.
///
/// ## Example
/// ```ignore
/// use exomat::harness::env::transform_env_list;
///
/// let list = vec![
///     vec!["VAR1".to_string(), "A".to_string(), "B".to_string()],
///     vec![
///         "VAR2".to_string(),
///         "42".to_string(),
///         "24".to_string(),
///         "44".to_string(),
///     ],
/// ];
///
/// let new_map = transform_env_list(&list).unwrap();
///
/// assert_eq!(new_map.len(), 2);
/// assert_eq!(*new_map.get("VAR1").unwrap(), vec!["A".to_string(), "B".to_string()]);
/// assert_eq!(*new_map.get("VAR2").unwrap(), vec!["42".to_string(), "24".to_string(), "44".to_string()]);
/// ```
///
/// ## Errors
/// - Returns an `EnvError` if `old_list` is empty
fn transform_env_list(old_list: &Vec<Vec<String>>) -> Result<EnvVarList> {
    if old_list.is_empty() {
        return Err(Error::EnvError {
            reason: "Cannot transform empty env list.".to_string(),
        });
    }

    let mut transformed: EnvVarList = HashMap::new();

    for occurence in old_list {
        let mut val = occurence.clone();
        let key = val.remove(0);

        transformed.insert(key, val);
    }

    Ok(transformed)
}

/// Fetch and load existing environment variables from .env file preserving file names
///
/// ## Errors and Panics
/// - Panics if `from` could not be read
/// - Returns an `EnvError` if something went wrong during the deserialization of envs
pub fn get_existing_envs_by_fname(from: &PathBuf) -> Result<HashMap<String, Environment>> {
    let mut envs: HashMap<String, Environment> = HashMap::new();

    // if there are .env files present, read existing vars from them
    if let Some(env_files) = fetch_env_files(from) {
        for file in env_files {
            let envs_in_file = Environment::from_file(&file)?;
            envs.insert(
                file.file_name()
                    .expect("file name must not be empty")
                    .to_str()
                    .expect("file name must be utf8")
                    .to_string(),
                envs_in_file,
            );
        }
    }

    Ok(envs)
}

/// Checks every first String of every Vector for a valid name.
///
/// "Environment variable names [...] consist solely of uppercase letters, digits,
/// and the underscore [...] and do not begin with a digit."
///
/// ## Errors and Panics
/// - Returns an EnvError on invalid names
/// - Panics if any Vec<String> is empty (or the first item cannot be extracted)
fn check_env_names(env_list: &[Vec<String>]) -> Result<()> {
    let re_env_name = Regex::new(r"^[A-Z_][0-9A-Z_]*$").expect("Could not create Regex");

    let invalid: Vec<&String> = env_list
        .iter()
        .map(|env_vec| env_vec.first().expect("Could not get env var name")) // get first item in Vector
        .filter(|env_name| re_env_name.captures(env_name).is_none()) // collect names that do not match regex
        .collect();

    match invalid.is_empty() {
        false => Err(Error::EnvError {
            reason: format!("Invalid environment variable name(s), only upper case alphanumeric and _ allowed: {invalid:?}").replace("\"", "'"),
        }),
        true => Ok(()),
    }
}

fn generate_environments(
    env_path: PathBuf,
    to_add: Vec<Vec<String>>,
    to_append: Vec<Vec<String>>,
    to_remove: Vec<Vec<String>>,
) -> Result<()> {
    let mut env = EnvironmentContainer::from_files(&env_path)?;

    // edit existing envs
    if !to_add.is_empty() {
        env.add_environments(to_add)?;
    }

    if !to_append.is_empty() {
        env.append_to_environments(to_append)?;
    }

    if !to_remove.is_empty() {
        env.remove_from_environments(to_remove)?;
    }

    // remove existing env files
    for entry in std::fs::read_dir(&env_path)? {
        let entry = entry?;
        std::fs::remove_file(entry.path())?;
    }

    // serialize new env files
    env.serialize_envs(&env_path)
}

/// print a pretty table of all configured environments in env_path
fn print_all_environments(env_path: PathBuf) -> Result<()> {
    let all_envs_by_fname = get_existing_envs_by_fname(&env_path)?;
    let all_envs_with_fname: Vec<(String, Environment)> = all_envs_by_fname
        .into_iter()
        .sorted_by_cached_key(|(k, _)| k.clone())
        .collect();

    let mut keys: Option<Vec<String>> = None;
    let mut table_builder = tabled::builder::Builder::default();
    info!("{} env files found", all_envs_with_fname.len());
    for (fname, env) in all_envs_with_fname {
        let this_env_keys: Vec<String> =
            env.get_variables().iter().map(|s| s.to_string()).collect();

        match keys {
            None => {
                table_builder.push_record(
                    std::iter::once("file".to_string())
                        .chain(this_env_keys.iter().map(|s| s.to_string())),
                );
                keys = Some(this_env_keys);
            }
            Some(ref old_keys) => {
                if *old_keys != this_env_keys {
                    return Err(Error::EnvError {
                        reason: "not all envs have the same keys".to_string(),
                    });
                }
            }
        }

        let keys = keys.as_ref().expect("keys must be initialized by now");
        // reorder values by list of keys
        table_builder.push_record(
            std::iter::once(fname.to_string()).chain(keys.iter().map(|s| {
                env.get_value(s)
                    .expect("key precondition check failed")
                    .to_string()
            })),
        );
    }

    let mut table = table_builder.build();
    table.with(tabled::settings::Style::sharp());
    // note: println to enforce newline after end
    println!("{table}");
    Ok(())
}

/// main entry point for env binary
///
/// Always operates in pwd
///
/// Performs the given operations by default.
/// If no operations are given, print a pretty table of all configured environments.
pub fn main(
    to_add: Vec<Vec<String>>,
    to_append: Vec<Vec<String>>,
    to_remove: Vec<Vec<String>>,
) -> Result<()> {
    let exp_source = find_marker_pwd(crate::MARKER_SRC)?;
    let env_path = exp_source.join(crate::SRC_ENV_DIR);

    match to_add.is_empty() && to_append.is_empty() && to_remove.is_empty() {
        true => print_all_environments(env_path),
        false => generate_environments(env_path, to_add, to_append, to_remove),
    }
}

#[cfg(test)]
mod tests {
    use rusty_fork::rusty_fork_test;
    use std::collections::HashMap;
    use tempfile::TempDir;

    use super::*;
    use crate::helper::archivist::{create_harness_dir, create_harness_file};
    use crate::helper::fs_names::*;

    #[test]
    fn fetch_envs_valid() {
        // create experiment source dir
        let mock_src = TempDir::new().unwrap();
        let mock_src = mock_src.path().to_path_buf();
        let mock_envs = create_harness_dir(&mock_src.join(SRC_ENV_DIR)).unwrap();

        create_harness_file(&mock_envs.join("42.env")).unwrap();
        create_harness_file(&mock_envs.join("foo.env")).unwrap();
        create_harness_file(&mock_envs.join("not_an_env")).unwrap();
        create_harness_dir(&mock_envs.join("not_a_file")).unwrap();

        let envs_found = fetch_env_files(&mock_envs).unwrap();

        assert_eq!(envs_found.len(), 2);
        assert!(envs_found.contains(&mock_envs.join("42.env")));
        assert!(envs_found.contains(&mock_envs.join("foo.env")));
        assert!(!envs_found.contains(&mock_envs.join("not_an_env")));
        assert!(!envs_found.contains(&mock_envs.join("not_a_file")));
    }

    #[test]
    fn fetch_envs_no_envs_dir() {
        // create experiment source dir
        let mock_src = TempDir::new().unwrap();
        let mock_src = mock_src.path().to_path_buf();

        assert!(fetch_env_files(&mock_src).is_none());
    }

    #[test]
    fn fetch_envs_no_env_files() {
        // create experiment source dir
        let mock_src = TempDir::new().unwrap();
        let mock_src = mock_src.path().to_path_buf();

        // create empty envs dir
        create_harness_dir(&mock_src.join(SRC_ENV_DIR)).unwrap();
        assert!(fetch_env_files(&mock_src.join(SRC_ENV_DIR)).is_none());
    }

    #[test]
    fn env_assemble_with_none() {
        let given = Environment::new();
        let to_add = HashMap::new();

        // should not throw (?)
        assert!(try_assemble_all(&given, &to_add).is_ok());
    }

    #[test]
    fn env_assemble_with_given() {
        let given = Environment::from_env_list(vec![("1".to_string(), "a".to_string())]);
        let to_add = HashMap::new();

        let assembled = try_assemble_all(&given, &to_add).unwrap();

        // should only contain the already given vars with nothing changed
        assert_eq!(assembled.len(), 1);
        assert!(assembled.contains(&given));
    }

    #[test]
    fn env_assemble_with_to_add() {
        let given = Environment::new();
        let to_add = HashMap::from([("1".to_string(), vec!["a".to_string()])]);

        let assembled = try_assemble_all(&given, &to_add).unwrap();

        // should contain the only possible variant from to_add
        assert_eq!(assembled.len(), 1);
        assert!(assembled.contains(&Environment::from_env_list(vec![(
            "1".to_string(),
            "a".to_string()
        )])));
    }

    #[test]
    fn env_assemble_with_one() {
        // Note: assembling with multiple values is tested in doctest

        let given = Environment::from_env_list(vec![("1".to_string(), "a".to_string())]);
        let to_add = HashMap::from([("2".to_string(), vec!["b".to_string()])]);

        let assembled = try_assemble_all(&given, &to_add).unwrap();

        assert_eq!(assembled.len(), 1);
        assert!(assembled.contains(&Environment::from_env_list(vec![
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "b".to_string()),
        ])));
    }

    #[test]
    fn env_validate_names() {
        // correct names
        let valid_list: Vec<Vec<String>> = vec![
            vec![String::from("VALID"), String::from("val")],
            vec![String::from("ALSO_VALID123_4"), String::from("val")],
            vec![String::from("_FOO_"), String::from("val")],
        ];
        assert!(check_env_names(&valid_list).is_ok());

        // starts with number
        let invalid_number: Vec<Vec<String>> = vec![vec![String::from("1"), String::from("val")]];
        assert!(check_env_names(&invalid_number).is_err());

        // includes lowercase
        let invalid_lowercase: Vec<Vec<String>> =
            vec![vec![String::from("INvALID"), String::from("val")]];
        assert!(check_env_names(&invalid_lowercase).is_err());

        // includes forbidden characters
        let invalid_characters: Vec<Vec<String>> =
            vec![vec![String::from("FOO,.-!§$&()?#~'<"), String::from("val")]];
        assert!(check_env_names(&invalid_characters).is_err());

        // more invalid characters (only whitespace)
        let invalid_whitespace: Vec<Vec<String>> =
            vec![vec![String::from(" "), String::from("val")]];
        assert!(check_env_names(&invalid_whitespace).is_err());

        // empty string
        let invalid_empty: Vec<Vec<String>> = vec![vec![String::from(""), String::from("val")]];
        assert!(check_env_names(&invalid_empty).is_err());
    }

    #[test]
    fn env_try_assemble() {
        let given = Environment::from_env_list(vec![("1".to_string(), "a".to_string())]);
        let to_add = HashMap::from([
            ("2".to_string(), vec!["b".to_string(), "c".to_string()]),
            ("3".to_string(), vec!["42".to_string(), "43".to_string()]),
        ]);

        let assembled = try_assemble_all(&given, &to_add).unwrap();
        assert_eq!(assembled.len(), 4);

        // all possible combinations of values that should be formed
        assert!(assembled.contains(&Environment::from_env_list(vec![
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "b".to_string()),
            ("3".to_string(), "42".to_string()),
        ])));

        assert!(assembled.contains(&Environment::from_env_list(vec![
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "b".to_string()),
            ("3".to_string(), "43".to_string()),
        ])));

        assert!(assembled.contains(&Environment::from_env_list(vec![
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "c".to_string()),
            ("3".to_string(), "42".to_string()),
        ])));

        assert!(assembled.contains(&Environment::from_env_list(vec![
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "c".to_string()),
            ("3".to_string(), "43".to_string()),
        ])));
    }

    #[test]
    fn env_transform_list() {
        let list = vec![
            vec!["VAR1".to_string(), "A".to_string(), "B".to_string()],
            vec![
                "VAR2".to_string(),
                "42".to_string(),
                "24".to_string(),
                "44".to_string(),
            ],
        ];

        let new_map = transform_env_list(&list).unwrap();

        assert_eq!(new_map.len(), 2);
        assert_eq!(
            *new_map.get("VAR1").unwrap(),
            vec!["A".to_string(), "B".to_string()]
        );
        assert_eq!(
            *new_map.get("VAR2").unwrap(),
            vec!["42".to_string(), "24".to_string(), "44".to_string()]
        );
    }

    rusty_fork_test! {
        #[test]
        fn env_e2e() {
            // create ouput dir (with empty envs/ dir)
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::fs::create_dir(tmpdir.join(SRC_ENV_DIR)).unwrap();
            std::fs::File::create_new(&tmpdir.join(MARKER_SRC)).unwrap();

            std::env::set_current_dir(&tmpdir).unwrap();

            let to_add = vec![vec!["VAR".to_string(), "VAL".to_string()]];
            let to_append = vec![vec!["VAR".to_string(), "FOO".to_string()]];
            let to_remove = vec![vec!["VAR".to_string(), "FOO".to_string()]];

            // check that no error occurs
            main( to_add, to_append, to_remove).unwrap()
        }

        #[test]
        fn deserialize() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();
            std::env::set_current_dir(&tmpdir).unwrap();

            // write in non-alphabetic order
            std::fs::write("two.env", "FOO=baz").unwrap();
            std::fs::write("01.env", "FOO=bar").unwrap();

            let expected_env_bar = Environment::from_env_list(vec![("FOO".to_string(), "bar".to_string())]);
            let expected_env_baz = Environment::from_env_list(vec![("FOO".to_string(), "baz".to_string())]);

            let all_envs_with_fname = get_existing_envs_by_fname(&PathBuf::from(".")).unwrap();
            assert_eq!(
                all_envs_with_fname,
                HashMap::from([
                    ("01.env".to_string(), expected_env_bar.clone()),
                    ("two.env".to_string(), expected_env_baz.clone())]));

            let all_envs_no_fname = EnvironmentContainer::from_files(&PathBuf::from(".")).unwrap();
            assert_eq!(
                *all_envs_no_fname.to_env_list(),
                vec![expected_env_bar, expected_env_baz]);
        }
    }

    #[test]
    fn update_exp_src_env() {
        let tmpdir = TempDir::new().unwrap();
        let src_dir = tmpdir.path().to_path_buf();
        let envs_dir = src_dir.join(SRC_ENV_DIR);
        let src_dir_str = src_dir.canonicalize().unwrap().display().to_string();
        std::fs::create_dir(&envs_dir).unwrap();

        // Write a .env file with an incorrect EXP_SRC_DIR value
        let env_file_path = envs_dir.join("test.env");
        std::fs::write(&env_file_path, "EXP_SRC_DIR=\"/wrong/path\"\nFOO=1").unwrap();

        // Updates EXP_SRC_DIR and leave FOO
        validate_src_env(&src_dir).unwrap();
        let envs = Environment::from_file(&env_file_path).unwrap();
        assert_eq!(envs.get_value("EXP_SRC_DIR"), Some(&src_dir_str));
        assert_eq!(envs.get_value("FOO"), Some(&"1".to_string()));

        // Doesn't break on valid EXP_SRC_DIR
        validate_src_env(&src_dir).unwrap();
        let envs = Environment::from_file(&env_file_path).unwrap();
        assert_eq!(envs.get_value("EXP_SRC_DIR"), Some(&src_dir_str));
        assert_eq!(envs.get_value("FOO"), Some(&"1".to_string()));

        // Adds env on missing EXP_SRC_DIR
        let env_file_path2 = envs_dir.join("test2.env");
        std::fs::write(&env_file_path2, "FOO=2").unwrap();

        validate_src_env(&src_dir).unwrap();
        let envs = Environment::from_file(&env_file_path2).unwrap();
        assert_eq!(envs.get_value("EXP_SRC_DIR"), Some(&src_dir_str));
        assert_eq!(envs.get_value("FOO"), Some(&"2".to_string()));
    }
}
