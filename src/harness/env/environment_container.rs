//! Implementation of the EnvironmentContainer struct

use log::{debug, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::environment::Environment;
use super::{
    assert_exists, check_env_vars, get_existing_environments_by_fname, to_env_list,
    try_assemble_all, EnvList,
};
use crate::helper::errors::{Error, Result};

/// Used to decide how an env should be edited
enum EditMode {
    Append,
    Remove,
}

/// List of multiple env files
#[derive(Debug)]
pub struct EnvironmentContainer {
    environment_list: Vec<Environment>,
}

impl EnvironmentContainer {
    /// Creates a new, empty EnvironmentContainer
    pub fn new() -> Self {
        EnvironmentContainer {
            environment_list: vec![],
        }
    }

    /// Fetch and deserialize existing environment variables from (multiple) .env files.
    ///
    /// Might return an empty EnvironmentContainer.
    /// Delegates to get_existing_envs_by_fname(), has same errors & panics.
    pub fn from_files(from: &PathBuf) -> Result<Self> {
        let environments_by_fname = get_existing_environments_by_fname(from)?;

        // create an Environment from each file
        Ok(EnvironmentContainer {
            environment_list: environments_by_fname
                .into_iter()
                .map(|(_, value)| value)
                .collect::<Vec<Environment>>(),
        })
    }

    /// Returns a new EnvironmentContainer from the content of `list`.
    pub fn from_env_list(list: Vec<Environment>) -> Self {
        EnvironmentContainer {
            environment_list: list,
        }
    }

    /// Returns a list of all Environments currently set in this EnvironmentContainer.
    pub fn to_env_list(&self) -> &Vec<Environment> {
        &self.environment_list
    }

    /// Writes all currently defined envs to `exp_src_envs/[i].env`.
    ///
    /// Will create each file if it does not exist and will entirely replace its
    /// contents if it does.
    /// This will fail if any parent directories of `exp_src_envs` to not exist.
    ///
    /// ## Errors
    /// - Returns an EnvError if writing failed
    pub fn serialize_environments(&self, exp_src_envs: &Path) -> Result<()> {
        let leading_zeros = self.environment_list.len().to_string().len();

        for (counter, environment) in self.environment_list.iter().enumerate() {
            let env_file_name = format!("{:0lz$}.env", counter, lz = leading_zeros);
            let file_path = &exp_src_envs.join(&env_file_name);

            environment.to_file(&file_path)?;
        }

        Ok(())
    }

    /// Takes existing envs and combines them with the envs from `to_add`.
    /// Does not overwrite existing envs.
    ///
    /// # Errors and Panics
    /// - Panics if `to_add` is empty
    /// - Panics if an inner vector has <= 1 elemets (variable without value)
    /// - Same Errors and Panics as `check_env_names()`
    /// - Returns an `EnvError` if a variable from `to_add` is already set
    pub fn add_environments(&mut self, to_add: Vec<Vec<String>>) -> Result<()> {
        // check to_add
        assert!(!to_add.is_empty(), "No envs to add. Aborting.");
        to_add
            .iter()
            .for_each(|v| assert!(v.len() > 1, "Found variable without value. Aborting."));

        check_env_vars(&to_add)?;

        // collect all envs to combine
        let to_add: EnvList = to_env_list(&to_add)?;

        // combine them, produces list of all env files with content
        if self.environment_list.is_empty() {
            self.environment_list = try_assemble_all(&Environment::new(), &to_add)?;
        } else {
            let mut new_list = vec![];

            for file in &self.environment_list {
                for var in to_add.keys() {
                    if file.contains_env_var(var) {
                        return Err(Error::EnvError {
                            reason: format!("Env var '{var}' is already set"),
                        });
                    }
                }

                new_list.extend(try_assemble_all(&file, &to_add)?);
            }

            self.environment_list = new_list;
        };

        Ok(())
    }

    /// Appends all values from `to_append` to the existing variables.
    ///
    /// There are two cases where nothing will be changed:
    /// - `to_append` is empty
    /// - an inner vector in `to_append` is empty (only the corresponding variable is
    ///   ignored, all other changes will still go through)
    ///
    /// ## Errors
    /// - Returns an `EnvError` if a variable from `to_append` does not exist yet.
    pub fn append_to_environments(&mut self, to_append: Vec<Vec<String>>) -> Result<()> {
        if to_append.is_empty() {
            return Ok(());
        }

        // check to_append, needs to happen before transforming
        to_append.iter().filter(|v| v.len() <= 1).for_each(|v| {
            warn!(
                "Cannot edit variable without value. Skipping {}.",
                v.first().unwrap()
            )
        });

        // collect all existing envs
        let to_append: EnvList = to_env_list(&to_append)?;

        // env exists?
        for var in to_append.keys() {
            assert_exists(&self.environment_list, |env_file| {
                env_file.contains_env_var(var)
            })
            .map_err(|e| Error::EnvError {
                reason: format!("Variable {var} cannot be edited: {e}"),
            })?;
        }

        // combine them, sets self.environment_list
        self.try_edit_values(&to_append, EditMode::Append)
    }

    /// Remove a variable from all Environments.
    ///
    /// Removes either a whole variable with all its values, or some values of a variable,
    /// depending on the content of `to_remove`. For example:
    /// ```notest
    /// env_container.remove_from_environments([vec!["FOO", "1", "2"], vec!["BAR"]])
    /// // removes any mentions of `FOO="1"`
    /// // removes any mentions of `FOO="2"`
    /// // removes any mentions of `BAR`, no matter the value
    /// ```
    ///
    /// > assuming "FOO" with at least its values "1" and "2", as well as "BAR" with
    /// > any values, are present in at least one environment.
    ///
    /// `to_remove` may be empty, nothing will be changed in that case.
    ///
    /// ## Errors
    /// - Returns an `EnvError` if any variable or value cannot be edited
    pub fn remove_from_environments(&mut self, to_remove: Vec<Vec<String>>) -> Result<()> {
        if to_remove.is_empty() {
            return Ok(());
        }

        // collect all existing envs
        let to_remove: EnvList = to_env_list(&to_remove)?;

        for (var, vals) in &to_remove {
            // var exists?
            assert_exists(&self.environment_list, |env_file| {
                env_file.contains_env_var(var)
            })
            .map_err(|e| Error::EnvError {
                reason: format!("Variable {var} cannot be edited: {e}"),
            })?;

            // vals exists?
            for val in vals {
                assert_exists(&self.environment_list, |env_file| {
                    env_file.get_env_val(var).unwrap().contains(val)
                })
                .map_err(|e| Error::EnvError {
                    reason: format!("Value {val} of {var} cannot be edited: {e}"),
                })?;
            }
        }

        // combine them, produces list of all env files with content
        self.try_edit_values(&to_remove, EditMode::Remove)
    }

    /// Edit existing environment variables.
    ///
    /// Depending on `edit_mode` it will:
    /// - `EditMode::Append`:
    ///   Add all values in `to_edit` to the list of possible values.
    /// - `EditMode::Remove`:
    ///   Remove all values in `to_edit` from the list of possible values.
    ///   Variables with empty value lists will then also be removed.
    ///
    /// Then calls on `try_assemble_all` to generate a "list of files" so to say, which
    /// contains all possible combinations of all values.
    /// This list will replace the current `self.environment_list`.
    ///
    /// Duplicate values will be removed before creating this list.
    ///
    /// # Panics
    /// - panics if a key from `to_edit` cannot be found in `self.environment_list`
    fn try_edit_values(&mut self, to_edit: &EnvList, edit_mode: EditMode) -> Result<()> {
        let mut possible_envs: EnvList = HashMap::new();

        // create a list of all possible values from all given files
        // collect values with the same key in one Vec
        for env_file_content in &self.environment_list {
            for (var, val) in env_file_content.to_env_list() {
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
                let vars_to_remove = helper_remove_env_vals(&mut possible_envs, to_edit)?;

                // remove vars that don't have values anymore
                for var in vars_to_remove {
                    assert!(possible_envs.remove_entry(&var).is_some());
                }
            }
        }

        // assemble files that need to be created, return
        self.environment_list = try_assemble_all(&Environment::new(), &possible_envs)?;
        Ok(())
    }

    /// Adds the variables from `new_environment` to each Environment in this container.
    pub fn extend_environments(&mut self, new_environment: &Environment) {
        self.environment_list
            .iter_mut()
            .for_each(|combo| combo.extend_envs(new_environment));
    }

    /// Number of Environments defined in this EnvironmentContainer.
    pub fn environment_count(&self) -> u64 {
        self.environment_list.len() as u64
    }
}

/// Remove any value of a key given in `to_edit` from `possible_envs`.
///
/// If a key in `to_edit` has an empty value, it will be returned in a vector. The
/// same thing happens if the last value of a key is deleted by this function.
///
/// ## Errors
/// - Returns an `EnvError` if a key or a value from `to_edit` is not found in `possible_envs`
fn helper_remove_env_vals(possible_envs: &mut EnvList, to_edit: &EnvList) -> Result<Vec<String>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    #[should_panic(expected = "No envs to add")]
    fn env_add_empty() {
        let mut env = EnvironmentContainer::new();
        let to_add: Vec<Vec<String>> = Vec::new();

        // should panic, because to_add is empty
        let _ = env.add_environments(to_add);
    }

    #[test]
    #[should_panic]
    fn env_add_no_val() {
        let mut env = EnvironmentContainer::new();
        let to_add = vec![vec!["VAR".to_string()]];

        let _ = env.add_environments(to_add);
    }

    #[test]
    fn env_add_repeat_env() {
        let mut env = EnvironmentContainer::new();
        let to_add = vec![vec!["VAR".to_string(), "VAL".to_string()]];
        env.add_environments(to_add).unwrap();

        // env was written
        assert_eq!(
            env.environment_list.first().unwrap().get_env_val("VAR"),
            Some(&"VAL".to_string())
        );

        // appending a new value to an existing one should fail
        let to_add = vec![vec![
            "VAR".to_string(),
            "VAL".to_string(),
            "VAL2".to_string(),
        ]];
        assert!(env.add_environments(to_add).is_err());
    }

    #[test]
    fn env_add_multiple() {
        // add to empty EnvironmentContainer
        let mut env = EnvironmentContainer::new();
        let to_add = vec![
            vec!["VAR1".to_string(), "VAL1".to_string(), "VAL11".to_string()],
            vec!["VAR2".to_string(), "VAL2".to_string(), "VAL22".to_string()],
        ];
        env.add_environments(to_add).unwrap();

        assert_eq!(env.environment_count(), 4);
        assert!(env.environment_list.iter().all(|environment| {
            environment.contains_env_var("VAR1") && environment.contains_env_var("VAR2")
        }));

        // add to non-empty EnvironmentContainer
        let to_add = vec![vec![
            "VAR3".to_string(),
            "VAL3".to_string(),
            "VAL33".to_string(),
        ]];
        env.add_environments(to_add).unwrap();

        assert_eq!(env.environment_count(), 8);
        assert!(env.environment_list.iter().all(|environment| {
            environment.contains_env_var("VAR1")
                && environment.contains_env_var("VAR2")
                && environment.contains_env_var("VAR3")
        }))
    }

    #[test]
    #[should_panic(expected = "Item does not exist.")]
    fn env_append_no_preexisting() {
        let mut env = EnvironmentContainer::new();

        // don't set any variables, try to edit
        let to_append = vec![vec!["VAR1".to_string(), "VALUE1".to_string()]];
        env.append_to_environments(to_append).unwrap(); //panic here
    }

    #[test]
    fn env_append_valid() {
        // list with "VAR"
        let mut env =
            EnvironmentContainer::from_env_list(vec![Environment::from_env_list(vec![(
                "VAR".to_string(),
                "VAL".to_string(),
            )])]);

        // edit "VAR"
        let to_append = vec![vec!["VAR".to_string(), "ANOTHER".to_string()]];
        env.append_to_environments(to_append).unwrap();

        // check "VAR", has to be set to "VAL" once and to "ANOTHER" once
        assert_eq!(env.environment_count(), 2);
        let res_first = env
            .environment_list
            .first()
            .unwrap()
            .get_env_val("VAR")
            .unwrap();
        let res_last = env
            .environment_list
            .last()
            .unwrap()
            .get_env_val("VAR")
            .unwrap();

        assert_eq!(res_first, &"ANOTHER".to_string());
        assert_eq!(res_last, &"VAL".to_string());
    }

    #[test]
    fn env_append_no_value() {
        // list with "VAR"
        let mut env = EnvironmentContainer::from_env_list(vec![Environment::from_env_list(vec![
            ("VAR1".to_string(), "VAL1".to_string()),
            ("VAR2".to_string(), "VAL2".to_string()),
        ])]);

        // edit "VAR1", but not "VAR2"
        let to_append = vec![
            vec!["VAR1".to_string(), "VALUE1".to_string()],
            vec!["VAR2".to_string()],
        ];
        env.append_to_environments(to_append).unwrap();

        // expected: no error, value of VAR1 changed but VAR2 not touched
        assert_eq!(env.environment_count(), 2);
        let env1 = env.environment_list.first().unwrap();
        let env2 = env.environment_list.last().unwrap();

        assert_eq!(env1.get_env_val("VAR1").unwrap(), &"VAL1".to_string());
        assert_eq!(env1.get_env_val("VAR2").unwrap(), &"VAL2".to_string());
        assert_eq!(env2.get_env_val("VAR1").unwrap(), &"VALUE1".to_string());
        assert_eq!(env2.get_env_val("VAR2").unwrap(), &"VAL2".to_string());
    }

    #[test]
    #[should_panic(expected = "Item does not exist.")]
    fn env_remove_no_preexisting() {
        // list with "VAR"
        let mut env = EnvironmentContainer::new();

        // don't set any variables, try to edit
        let to_remove = vec![vec!["VAR1".to_string(), "VALUE1".to_string()]];
        env.append_to_environments(to_remove).unwrap(); //panic here
    }

    #[test]
    fn env_remove_valid() {
        // list with "VAR1" and "VAR2"
        let mut env = EnvironmentContainer::from_env_list(vec![
            Environment::from_env_list(vec![
                ("VAR1".to_string(), "VAL".to_string()),
                ("VAR2".to_string(), "VAL".to_string()),
            ]),
            Environment::from_env_list(vec![
                ("VAR1".to_string(), "VALUE".to_string()),
                ("VAR2".to_string(), "VAL".to_string()),
            ]),
        ]);

        let to_remove = vec![
            vec!["VAR1".to_string(), "VALUE".to_string()], // remove value
            vec!["VAR2".to_string()],                      // remove variable
        ];

        // remove
        env.remove_from_environments(to_remove).unwrap();

        assert_eq!(env.environment_count(), 1);
        let env1 = env.environment_list.first().unwrap();

        assert_eq!(env1.get_env_val("VAR1").unwrap(), &"VAL".to_string());
        assert!(env1.get_env_val("VAR2").is_none());
    }

    #[test]
    fn env_serialize() {
        // list with Environments that contain content
        let env = EnvironmentContainer::from_env_list(vec![Environment::from_env_list(vec![(
            "VAR".to_string(),
            "VAL".to_string(),
        )])]);

        // list with a lot of Environments (10)
        let many_env = EnvironmentContainer::from_env_list(vec![Environment::new(); 11]);

        let tmpdir = TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();

        // expecting "0.env" with the content VAR="VAL"
        env.serialize_environments(&tmpdir).unwrap();
        let content = std::fs::read_to_string(tmpdir.join("0.env")).unwrap();
        assert!(!tmpdir.join("1.env").is_file());
        assert_eq!(content, "VAR=\"VAL\"");

        // expecting 10 files, from "00.env" to "10.env" without content
        many_env.serialize_environments(&tmpdir).unwrap();
        let content0 = std::fs::read_to_string(tmpdir.join("00.env")).unwrap();
        let content1 = std::fs::read_to_string(tmpdir.join("10.env")).unwrap();
        assert!(!tmpdir.join("11.env").is_file());
        assert!(content0.is_empty());
        assert!(content1.is_empty());
    }
}
