//! Implementation of the Environment struct

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::helper::errors::{Error, Result};

/// Represents one environment file
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    envs: HashMap<String, String>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<&PathBuf> for Environment {
    type Error = Error;

    /// Constructs a new Environment with all variables and values from a file.
    /// Does not include process environment variables.
    ///
    /// ## Parameters
    /// `file` needs to be a valid env file, see Errors and Panics
    ///
    ///  ## Errors and Panics
    /// - Panics if `file` does not end in ".env"
    /// - Returns an `EnvError` if `file` isn't a valid .env file (defined by
    ///   the `dotenvy` crate)
    /// - Returns an `EnvError` if an error occured during parsing
    fn try_from(file: &PathBuf) -> Result<Self> {
        // check for .env extension
        if file.extension().unwrap() != "env" {
            return Err(Error::EnvError {
                reason: format!("env file with missing extension: {}", file.display()),
            });
        }

        let mut env = Environment::new();

        // Not using serde_envfile here, because it converts "VAR" to "var" :(
        for item in dotenvy::from_filename_iter(file)? {
            let (var, val) = item.map_err(|e| Error::EnvError {
                reason: e.to_string(),
            })?;

            env.envs.insert(var, val);
        }

        Ok(env)
    }
}

impl From<Vec<(String, String)>> for Environment {
    /// Returns a new Environment with `list` as it's variables.
    fn from(env_list: Vec<(String, String)>) -> Self {
        env_list.into_iter().collect()
    }
}

impl FromIterator<(String, String)> for Environment {
    /// Collects `(variable, value)` pairs into a new Environment.
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        Environment {
            envs: iter.into_iter().collect(),
        }
    }
}

impl From<Environment> for HashMap<String, String> {
    /// Returns a map of all envs saved in `env`.
    fn from(env: Environment) -> Self {
        env.envs
    }
}

impl From<Environment> for HashMap<String, Vec<String>> {
    /// Returns a map with the env values of `env` in a vector
    fn from(env: Environment) -> Self {
        env.envs.into_iter().map(|(k, v)| (k, vec![v])).collect()
    }
}

impl IntoIterator for Environment {
    type Item = (String, String);
    type IntoIter = std::collections::hash_map::IntoIter<String, String>;

    /// Iterates over all `(variable, value)` pairs, consuming this Environment.
    fn into_iter(self) -> Self::IntoIter {
        self.envs.into_iter()
    }
}

impl<'a> IntoIterator for &'a Environment {
    type Item = (&'a String, &'a String);
    type IntoIter = std::collections::hash_map::Iter<'a, String, String>;

    /// Iterates over all `(variable, value)` pairs.
    fn into_iter(self) -> Self::IntoIter {
        self.envs.iter()
    }
}

impl Environment {
    /// Constructs an empty Environment
    pub fn new() -> Self {
        Environment {
            envs: HashMap::new(),
        }
    }

    /// Loads and returns all currently loaded environment variables, complete with variables
    /// defined in `env_file`.
    ///
    /// If a variable set in `env_file` is already loaded, it will be overwritten with
    /// the value given in `env_file`.
    ///
    /// ## Example
    /// ```
    /// use exomat::harness::env::environment::Environment;
    ///
    /// // create an .env file with TEST=true
    /// let mock_env_file = tempfile::Builder::new()
    ///     .suffix(".env")
    ///     .tempfile()
    ///     .unwrap();
    /// let mock_env_file = mock_env_file.path().to_path_buf();
    /// std::fs::write(&mock_env_file, "TEST=true").unwrap();
    ///
    /// let envs = Environment::load_from_file(&mock_env_file).unwrap();
    ///
    /// // load_from_file returns **all** currently loaded envs, so there will be more
    /// // than just the one we set
    /// assert!(envs.get_env_vars().len() > 1);
    ///
    /// // load_from_file has created a variable called "TEST" with the value "true"
    /// assert!(envs.contains_env_var("TEST"));
    /// assert_eq!(envs.get_env_val("TEST"), Some(&String::from("true")));
    ///
    /// // and it is actually loaded
    /// assert_eq!(dotenvy::var("TEST").unwrap(), "true");
    /// ```
    pub fn load_from_file(env_file: &Path) -> Result<Self> {
        dotenvy::from_path_override(env_file)?;
        Ok(dotenvy::vars().collect())
    }

    /// Serialize current envs to `file_path`.
    ///
    /// Will create a new file if `file_path` does not exist and will overwrite it if it does.
    /// This will fail if any parent directories of `file_path` do not exist.
    ///
    /// ## Errors
    /// - Returns an `EnvError` if writing failed
    pub fn to_file(&self, file_path: &Path) -> Result<()> {
        serde_envfile::to_file(file_path, &self.envs).map_err(|e| Error::EnvError {
            reason: e.to_string(),
        })
    }

    /// Returns `true` if the variable exists in this Environment.
    ///
    /// Does not check the value associated with the variable. A variable with
    /// empty values also returns `true` here.
    pub fn contains_env_var(&self, var: &str) -> bool {
        self.envs.contains_key(var)
    }

    /// Insert a variable into this Environment.
    ///
    /// If the variable already exists, only the value will be updated.
    pub fn add_env(&mut self, var: String, val: String) {
        self.envs.insert(var, val);
    }

    /// Append all variables from `other_env` onto this Environment.
    pub fn extend_envs(&mut self, other_env: &Environment) {
        self.envs.extend(other_env.envs.clone());
    }

    /// Returns the value associated with `var`.
    ///
    /// Will return `None` if `var` is  not set.
    pub fn get_env_val(&self, var: &str) -> Option<&String> {
        self.envs.get(var)
    }

    /// Returns a list of all defined variables without their values.
    pub fn get_env_vars(&self) -> Vec<&String> {
        self.envs.keys().collect()
    }
}
