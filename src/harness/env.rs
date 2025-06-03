//! harness env subcommand

use itertools::Itertools;
use log::{debug, info, trace, warn};
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::helper::archivist::find_marker_pwd;
use crate::helper::errors::{Error, Result};

/// Used to decide how an env should be edited
enum EditMode {
    Append,
    Remove,
}

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

/// map with all variables and one possible value (content of one .env file)
///
/// ## Example
/// - `0.env`: FOO=true, BAR=1
/// - `1.env`: FOO=true, BAR=2
///
/// can be encoded with EnvFileContent like this:
/// - `["FOO" = "true", "BAR" = "1"]`
/// - `["FOO" = "true", "BAR" = "2"]`
pub type EnvFileContent = HashMap<String, String>;

/// list of maps with variables (and values) taken from multiple .env files
///
/// ## Example
/// - `0.env`: FOO=true, BAR=1
/// - `1.env`: FOO=true, BAR=2
///
/// can be encoded in an EnvFileList like this:
/// - `[["FOO" = "true", "BAR" = "1"], ["FOO" = "true", "BAR" = "2"]]`
pub type EnvFileList = Vec<EnvFileContent>;

/// Loads and returns all currently loaded environment variables, complete with variables
/// defined in `env_file`.
///
/// If a variable set in `env_file` is already loaded, it will be overwritten with
/// the value given in `env_file`.
///
/// ## Example
/// ```
/// use exomat::harness::env::load_envs;
///
/// // create an .env file with TEST=true
/// let mock_env_file = tempfile::Builder::new()
///     .suffix(".env")
///     .tempfile()
///     .unwrap();
/// let mock_env_file = mock_env_file.path().to_path_buf();
/// std::fs::write(&mock_env_file, "TEST=true").unwrap();
///
/// let envs = load_envs(&mock_env_file).unwrap();
///
/// // load_envs returns **all** currently loaded envs, so there will be more than
/// // just the one we set
/// assert!(envs.len() > 1);
///
/// // load_envs has created a variable called "TEST" with the value "true"
/// assert!(envs.contains_key("TEST"));
/// assert_eq!(envs.get("TEST"), Some(&String::from("true")));
///
/// // and it is actually loaded
/// assert_eq!(dotenvy::var("TEST").unwrap(), "true");
/// ```
pub fn load_envs(env_file: &PathBuf) -> Result<EnvFileContent> {
    dotenvy::from_path_override(env_file)?;
    Ok(dotenvy::vars().collect())
}

/// Parses all variables and values from `file`.
///
/// ## Example
/// ```
/// use exomat::harness::env::deserialize_envs;
///
/// // create an .env file with TEST=true
/// let mock_env_file = tempfile::Builder::new()
///     .suffix(".env")
///     .tempfile()
///     .unwrap();
/// let mock_env_file = mock_env_file.path().to_path_buf();
/// std::fs::write(&mock_env_file, "TEST=true").unwrap();
///
/// let envs_in_file = deserialize_envs(&mock_env_file).unwrap();
///
/// assert_eq!(envs_in_file.len(), 1);
/// assert_eq!(envs_in_file.get("TEST"), Some(&String::from("true")));
///
/// // has not been loaded (use load_envs() for that purpose)
/// assert!(dotenvy::var("TEST").is_err());
/// ```
/// ## Errors and Panics
/// - Panics if `file` does not end in ".env"
/// - Returns an `EnvError` if `file` isn't a valid .env file (this does not include having
///   the correct extension) or if an error occured during parsing.
pub fn deserialize_envs(file: &PathBuf) -> Result<EnvFileContent> {
    // check for .env extension
    assert!(
        file.extension().unwrap() == "env",
        "env file with missing extension: {}",
        file.display()
    );

    let mut file_envs: EnvFileContent = HashMap::new();

    // Not using serde_envfile here, because it converts "VAR" to "var" :(
    for item in dotenvy::from_filename_iter(file)? {
        let (var, val) = item.map_err(|e| Error::EnvError {
            reason: e.to_string(),
        })?;

        file_envs.insert(var, val);
    }

    Ok(file_envs)
}

/// Writes all envs of each HashMap in `files_to_write` to `exp_src_envs/[i].env`.
///
/// Will each file if it does not exist and will entirely replace its
/// contents if it does.
/// This will fail if any parent directories of `exp_src_envs` to not exist.
///
/// ## Errors
/// - Returns an EnvError if writing failed
pub fn serialize_envs(exp_src_envs: &Path, files_to_write: &EnvFileList) -> Result<()> {
    let leading_zeros = files_to_write.len().to_string().len();

    for (counter, file_content) in files_to_write.iter().enumerate() {
        let env_file_name = format!("{:0lz$}.env", counter, lz = leading_zeros);
        let file_path = &exp_src_envs.join(env_file_name);

        serde_envfile::to_file(file_path, &file_content).map_err(|e| Error::EnvError {
            reason: e.to_string(),
        })?;
    }

    Ok(())
}

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

/// Reads all variables from any .env files found in 'exp_source/[SRC_ENV_DIR]/'.
/// Combines them with the values from `to_add` and creates a file for each
/// possible combination.
///
/// Might create new files or overwrite existing .env files in `exp_source`.
///
/// # Panics
/// - Panics if `to_add` is empty
pub fn add_environments(
    existing_envs: &EnvFileList,
    to_add: Vec<Vec<String>>,
) -> Result<EnvFileList> {
    // check to_add
    assert!(!to_add.is_empty(), "No env variables to add. Aborting.");
    to_add
        .iter()
        .for_each(|v| assert!(v.len() > 1, "Found variable without value. Aborting."));

    check_env_names(&to_add)?;

    // collect all envs to combine
    let to_add: EnvVarList = transform_env_list(&to_add)?;
    let mut files_to_write: EnvFileList = Vec::new();

    // combine them, produces list of all env files with content
    if existing_envs.is_empty() {
        files_to_write = try_assemble_all(&HashMap::new(), &to_add)?;
    } else {
        for file in existing_envs {
            for var in to_add.keys() {
                if file.contains_key(var) {
                    return Err(Error::EnvError {
                        reason: format!("Var '{var}' is already set"),
                    });
                }
            }

            match try_assemble_all(file, &to_add) {
                Ok(file_vars) => files_to_write.extend(file_vars),
                Err(e) => return Err(e),
            };
        }
    };

    Ok(files_to_write)
}

/// Reads all variables from any .env files found in 'exp_source/[SRC_ENV_DIR]/'.
/// Appends all values from `to_append` and creates a file for each possible combination.
///
/// Might create new files or overwrite existing .env files in `exp_source/[SRC_ENV_DIR]`.
///
/// There are two cases where nothing will be changed:
/// - `to_append` is empty
/// - an inner vector in `to_append` is empty (only the corresponding variable is
///   ignored, all other changes will still go through)
pub fn append_to_environments(
    existing_envs: &EnvFileList,
    to_append: Vec<Vec<String>>,
) -> Result<EnvFileList> {
    if to_append.is_empty() {
        return Ok(existing_envs.to_owned());
    }

    // check to_append, needs to happen before transforming
    to_append.iter().filter(|v| v.len() <= 1).for_each(|v| {
        warn!(
            "Cannot edit variable without value. Skipping {}.",
            v.first().unwrap()
        )
    });

    // collect all existing envs
    let to_append: EnvVarList = transform_env_list(&to_append)?;

    // env exists?
    for var in to_append.keys() {
        assert_exists(existing_envs, |env_file| env_file.contains_key(var)).map_err(|e| {
            Error::EnvError {
                reason: format!("Variable {var} cannot be edited: {e}"),
            }
        })?;
    }

    // combine them, produces list of all env files with content
    let files_to_write = try_edit_values(existing_envs, &to_append, EditMode::Append)?;
    Ok(files_to_write)
}

/// Reads all variables from any .env files found in 'exp_source/[SRC_ENV_DIR]/'.
/// Removes either a whole variable with all its values, or some values of a variable,
/// depending on the content of `to_remove`. For example:
///
/// `to_remove` = `[["FOO", "1", "2"], ["BAR"]]`
/// - removes any mentions of `FOO="1"`
/// - removes any mentions of `FOO="2"`
/// - removes any mentions of `BAR`, no matter the value
///
/// > assuming "FOO" with at least its values "1" and "2", as well as "BAR" with
/// > any values, are present in at least one environment in `exp_source/[SRC_ENV_DIR]/`.
///
/// Might remove or overwrite existing .env files in `exp_source/[SRC_ENV_DIR]`.
///
/// `to_remove` may be empty, nothing will be changed in that case.
pub fn remove_from_environments(
    existing_envs: &EnvFileList,
    to_remove: Vec<Vec<String>>,
) -> Result<EnvFileList> {
    if to_remove.is_empty() {
        return Ok(existing_envs.to_owned());
    }

    // collect all existing envs
    let to_remove: EnvVarList = transform_env_list(&to_remove)?;

    for (var, vals) in &to_remove {
        // var exists?
        assert_exists(existing_envs, |env_file| env_file.contains_key(var)).map_err(|e| {
            Error::EnvError {
                reason: format!("Variable {var} cannot be edited: {e}"),
            }
        })?;

        // vals exists?
        for val in vals {
            assert_exists(existing_envs, |env_file| {
                env_file.get(var).unwrap().contains(val)
            })
            .map_err(|e| Error::EnvError {
                reason: format!("Value {val} of {var} cannot be edited: {e}"),
            })?;
        }
    }

    // combine them, produces list of all env files with content
    let files_to_write = try_edit_values(existing_envs, &to_remove, EditMode::Remove)?;
    Ok(files_to_write)
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
fn try_assemble_all(given: &EnvFileContent, to_add: &EnvVarList) -> Result<EnvFileList> {
    // combine all values from to_add
    let mut combinations: EnvFileList = to_add
        .values()
        .multi_cartesian_product()
        .collect::<Vec<_>>() // list of all possible value combinations without keys
        .into_iter()
        .map(|val_combos| {
            to_add
                .keys()
                .cloned()
                .zip(val_combos.iter().map(|s| s.to_string())) // zip with keys
                .collect::<EnvFileContent>()
        })
        .collect::<EnvFileList>();

    trace!("Adding env combinations: {combinations:?}");

    // add existing variables to each list
    combinations
        .iter_mut()
        .for_each(|combo| combo.extend(given.clone()));

    debug!("Finished assembling environments: {combinations:?}");

    Ok(combinations)
}

/// Edit existing environment variables.
///
/// Depending on `edit_mode` it will:
/// - `EditMode::Append`:
///   Collect all existing variables and add all values in `to_edit` to the list of possible values.
/// - `EditMode::Remove`:
///   Collect all existing variables and remove all values from `to_edit` from the list of possible values.
///   Variables with empty value lists will then also be removed.
///
/// Then calls on `try_assemble_all` to generate a "list of files" so to say, which
/// contains all possible combinations of all values.
///
/// Duplicate values will be removed before creating this list.
///
/// # Panics
/// - panics if a key from `to_edit` cannot be found in `given`
fn try_edit_values(
    given: &EnvFileList,
    to_edit: &EnvVarList,
    edit_mode: EditMode,
) -> Result<EnvFileList> {
    let mut possible_envs: EnvVarList = HashMap::new();

    // create a list of all possible values from all given files
    // collect values with the same key in one Vec
    for env_file_content in given {
        for (var, val) in env_file_content {
            // push to value of entry "var"
            possible_envs
                .entry(var.clone())
                .or_default()
                .push(val.clone());
        }
    }

    // remove duplicates
    for values in possible_envs.values_mut() {
        values.sort();
        values.dedup();
    }

    debug!("All possible environment values: {possible_envs:?}");

    match edit_mode {
        EditMode::Append => {
            // add new values to the list, remove duplicates
            for (var, vals) in to_edit {
                let v = possible_envs.get_mut(var).unwrap();
                v.extend(vals.clone());

                v.sort();
                v.dedup();
            }
        }
        EditMode::Remove => {
            let vars_to_remove = helper_remove_env_values(&mut possible_envs, to_edit)?;

            // remove vars that don't have values anymore
            for var in vars_to_remove {
                assert!(possible_envs.remove_entry(&var).is_some());
            }
        }
    }

    // assemble files that need to be created, return
    try_assemble_all(&HashMap::new(), &possible_envs)
}

/// Remove any value of a key given in `to_edit` from `possible_envs`.
///
/// If a key in `to_edit` has an empty value, it will be returned in a vector. The
/// same thing happens if the last value of a key is deleted by this function.
///
/// ## Errors
/// - Returns an `EnvError` if a key or a value from `to_edit` is not found in `possible_envs`
fn helper_remove_env_values(
    possible_envs: &mut EnvVarList,
    to_edit: &EnvVarList,
) -> Result<Vec<String>> {
    let mut vars_to_remove = Vec::new();

    // remove values from list
    for (var, vals) in to_edit {
        let var_to_edit = possible_envs.get_mut(var).ok_or_else(|| Error::EnvError {
            reason: format!("Cannot remove values from {var}, it does not exist yet."),
        })?;

        for val in vals {
            let i = var_to_edit
                .iter()
                .position(|old_v| old_v == val)
                .ok_or_else(|| Error::EnvError {
                    reason: format!("Cannot remove value {val} from {var}, it does not exist yet."),
                })?;
            var_to_edit.remove(i);
        }

        // variable has no values or should explicitly be removed
        if var_to_edit.is_empty() || vals.is_empty() {
            vars_to_remove.push(var.to_owned());
        }
    }

    Ok(vars_to_remove)
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

/// Fetch and deserialize existing environment variables from .env files.
///
/// Might return an empty Vector.
/// Delegates to get_existing_envs_by_fname(), has same errors & panics.
pub fn get_existing_envs(from: &PathBuf) -> Result<EnvFileList> {
    let envs_by_fname = get_existing_envs_by_fname(from)?;
    Ok(envs_by_fname
        .into_iter()
        .sorted_by_key(|(key, _)| key.clone())
        .map(|(_, value)| value)
        .collect())
}

/// Fetch and load existing environment variables from .env file preserving file names
///
///
///
/// ## Errors and Panics
/// - Panics if `from` could not be read
/// - Returns an `EnvError` if something went wrong during the deserialization of envs
pub fn get_existing_envs_by_fname(from: &PathBuf) -> Result<HashMap<String, EnvFileContent>> {
    let mut envs: HashMap<String, EnvFileContent> = HashMap::new();

    // if there are .env files present, read existing vars from them
    if let Some(env_files) = fetch_env_files(from) {
        for file in env_files {
            let envs_in_file = deserialize_envs(&file)?;
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
    let mut to_serialize = get_existing_envs(&env_path)?;

    // edit existing envs
    if !to_add.is_empty() {
        to_serialize = add_environments(&to_serialize, to_add)?;
    }

    if !to_append.is_empty() {
        to_serialize = append_to_environments(&to_serialize, to_append)?;
    }

    if !to_remove.is_empty() {
        to_serialize = remove_from_environments(&to_serialize, to_remove)?;
    }

    // remove existing env files
    for entry in std::fs::read_dir(&env_path)? {
        let entry = entry?;
        std::fs::remove_file(entry.path())?;
    }

    // serialize new env files
    serialize_envs(&env_path, &to_serialize)
}

/// print a pretty table of all configured environments in env_path
fn print_all_environments(env_path: PathBuf) -> Result<()> {
    let all_envs_by_fname = get_existing_envs_by_fname(&env_path)?;
    let all_envs_with_fname: Vec<(String, EnvFileContent)> = all_envs_by_fname
        .into_iter()
        .sorted_by_cached_key(|(k, _)| k.clone())
        .collect();

    let mut keys: Option<Vec<String>> = None;
    let mut table_builder = tabled::builder::Builder::default();
    info!("{} env files found", all_envs_with_fname.len());
    for (fname, env) in all_envs_with_fname {
        let this_env_keys: Vec<String> = env.keys().sorted().map(|s| s.to_string()).collect();
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
                env.get(s)
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
        let given = HashMap::new();
        let to_add = HashMap::new();

        // should not throw (?)
        assert!(try_assemble_all(&given, &to_add).is_ok());
    }

    #[test]
    fn env_assemble_with_given() {
        let given = HashMap::from([("1".to_string(), "a".to_string())]);
        let to_add = HashMap::new();

        let assembled = try_assemble_all(&given, &to_add).unwrap();

        // should only contain the already given vars with nothing changed
        assert_eq!(assembled.len(), 1);
        assert!(assembled.contains(&given));
    }

    #[test]
    fn env_assemble_with_to_add() {
        let given = HashMap::new();
        let to_add = HashMap::from([("1".to_string(), vec!["a".to_string()])]);

        let assembled = try_assemble_all(&given, &to_add).unwrap();

        // should contain the only possible variant from to_add
        assert_eq!(assembled.len(), 1);
        assert!(assembled.contains(&HashMap::from([("1".to_string(), "a".to_string())])));
    }

    #[test]
    fn env_assemble_with_one() {
        // Note: assembling with multiple values is tested in doctest

        let given = HashMap::from([("1".to_string(), "a".to_string())]);
        let to_add = HashMap::from([("2".to_string(), vec!["b".to_string()])]);

        let assembled = try_assemble_all(&given, &to_add).unwrap();

        assert_eq!(assembled.len(), 1);
        assert!(assembled.contains(&HashMap::from([
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
    #[should_panic(expected = "No env variables to add")]
    fn env_add_empty() {
        let to_add: Vec<Vec<String>> = Vec::new();

        // should panic, because to_add is empty
        let _ = add_environments(&Vec::new(), to_add);
    }

    #[test]
    #[should_panic]
    fn env_add_no_val() {
        let to_add = vec![vec!["VAR".to_string()]];

        let _ = add_environments(&Vec::new(), to_add);
    }

    #[test]
    fn env_add_repeat_env() {
        let existing = Vec::new();
        let to_add = vec![vec!["VAR".to_string(), "VAL".to_string()]];
        let combined = add_environments(&existing, to_add).unwrap();

        // env was written
        assert_eq!(
            combined.first().unwrap().get("VAR"),
            Some(&"VAL".to_string())
        );

        // appending a new value to an existing one should fail
        let to_add = vec![vec![
            "VAR".to_string(),
            "VAL".to_string(),
            "VAL2".to_string(),
        ]];
        assert!(add_environments(&combined, to_add).is_err());
    }

    #[test]
    #[should_panic(expected = "Item does not exist.")]
    fn env_append_no_preexisting() {
        let existing = Vec::new();

        // don't set any variables, try to edit
        let to_append = vec![vec!["VAR1".to_string(), "VALUE1".to_string()]];
        append_to_environments(&existing, to_append).unwrap(); //panic here
    }

    #[test]
    fn env_append_valid() {
        // list with "VAR"
        let existing = vec![HashMap::from([("VAR".to_string(), "VAL".to_string())])];

        // edit "VAR"
        let to_append = vec![vec!["VAR".to_string(), "ANOTHER".to_string()]];
        let res = append_to_environments(&existing, to_append).unwrap();

        // check "VAR", has to be set to "VAL" once and to "ANOTHER" once
        assert_eq!(res.len(), 2);
        let res_first = res.first().unwrap().get("VAR").unwrap();
        let res_last = res.last().unwrap().get("VAR").unwrap();

        assert_eq!(res_first, &"ANOTHER".to_string());
        assert_eq!(res_last, &"VAL".to_string());
    }

    #[test]
    fn env_append_no_value() {
        // list with "VAR"
        let existing = vec![HashMap::from([
            ("VAR1".to_string(), "VAL1".to_string()),
            ("VAR2".to_string(), "VAL2".to_string()),
        ])];

        // edit "VAR1", but not "VAR2"
        let to_append = vec![
            vec!["VAR1".to_string(), "VALUE1".to_string()],
            vec!["VAR2".to_string()],
        ];
        let res = append_to_environments(&existing, to_append).unwrap();

        // expected: no error, value of VAR1 changed but VAR2 not touched
        assert_eq!(res.len(), 2);
        let res_first_1 = res.first().unwrap().get("VAR1").unwrap();
        let res_first_2 = res.first().unwrap().get("VAR2").unwrap();
        let res_last_1 = res.last().unwrap().get("VAR1").unwrap();
        let res_last_2 = res.last().unwrap().get("VAR2").unwrap();

        assert_eq!(res_first_1, &"VAL1".to_string());
        assert_eq!(res_first_2, &"VAL2".to_string());
        assert_eq!(res_last_1, &"VALUE1".to_string());
        assert_eq!(res_last_2, &"VAL2".to_string());
    }

    #[test]
    #[should_panic(expected = "Item does not exist.")]
    fn env_remove_no_preexisting() {
        // list with "VAR"
        let existing = Vec::new();

        // don't set any variables, try to edit
        let to_remove = vec![vec!["VAR1".to_string(), "VALUE1".to_string()]];
        append_to_environments(&existing, to_remove).unwrap(); //panic here
    }

    #[test]
    fn env_remove_valid() {
        // list with "VAR1" and "VAR2"
        let existing = vec![
            HashMap::from([
                ("VAR1".to_string(), "VAL".to_string()),
                ("VAR2".to_string(), "VAL".to_string()),
            ]),
            HashMap::from([
                ("VAR1".to_string(), "VALUE".to_string()),
                ("VAR2".to_string(), "VAL".to_string()),
            ]),
        ];

        let to_remove = vec![
            vec!["VAR1".to_string(), "VALUE".to_string()], // remove value
            vec!["VAR2".to_string()],                      // remove variable
        ];

        // remove
        let res = remove_from_environments(&existing, to_remove).unwrap();

        assert_eq!(res.len(), 1);
        assert!(res.first().unwrap().get("VAR2").is_none());

        let res_var1 = res.first().unwrap().get("VAR1").unwrap();
        assert_eq!(res_var1, &"VAL".to_string());
    }

    #[test]
    fn env_try_assemble() {
        let given = HashMap::from([("1".to_string(), "a".to_string())]);
        let to_add = HashMap::from([
            ("2".to_string(), vec!["b".to_string(), "c".to_string()]),
            ("3".to_string(), vec!["42".to_string(), "43".to_string()]),
        ]);

        let assembled = try_assemble_all(&given, &to_add).unwrap();
        assert_eq!(assembled.len(), 4);

        // all possible combinations of values that should be formed
        assert!(assembled.contains(&HashMap::from([
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "b".to_string()),
            ("3".to_string(), "42".to_string()),
        ])));

        assert!(assembled.contains(&HashMap::from([
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "b".to_string()),
            ("3".to_string(), "43".to_string()),
        ])));

        assert!(assembled.contains(&HashMap::from([
            ("1".to_string(), "a".to_string()),
            ("2".to_string(), "c".to_string()),
            ("3".to_string(), "42".to_string()),
        ])));

        assert!(assembled.contains(&HashMap::from([
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

            let expected_env_bar: EnvFileContent = HashMap::from([("FOO".to_string(), "bar".to_string())]);
            let expected_env_baz: EnvFileContent = HashMap::from([("FOO".to_string(), "baz".to_string())]);

            let all_envs_with_fname = get_existing_envs_by_fname(&PathBuf::from(".")).unwrap();
            assert_eq!(
                all_envs_with_fname,
                HashMap::from([
                    ("01.env".to_string(), expected_env_bar.clone()),
                    ("two.env".to_string(), expected_env_baz.clone())]));

            let all_envs_no_fname = get_existing_envs(&PathBuf::from(".")).unwrap();
            assert_eq!(
                all_envs_no_fname,
                vec![expected_env_bar, expected_env_baz]);
        }
    }
}
