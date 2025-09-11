//! Implementation of the Environment struct

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::helper::errors::{Error, Result};

/// Represents one environment file
///
/// Contains a list for envs from an environment file, and a list for exomat-internal envs.
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    envs: HashMap<String, String>,
    internal_envs: HashMap<String, String>,
}

impl Environment {
    /// Constructs an empty Environment
    pub fn new() -> Self {
        Environment {
            envs: HashMap::new(),
            internal_envs: HashMap::new(), //TODO: turn into an enum/ struct
        }
    }

    /// Constructs a new Environment with all variables and values from a file.
    /// Does not include process environment variables. Does not set internal
    /// variables.
    ///
    /// ## Parameters
    /// `file` needs to be a valid env file, see Errors and Panics
    ///
    ///  ## Errors and Panics
    /// - Panics if `file` does not end in ".env"
    /// - Returns an `EnvError` if `file` isn't a valid .env file (defined by
    ///   the `dotenvy` crate)
    /// - Returns an `EnvError` if an error occured during parsing
    pub fn from_file(file: &Path) -> Result<Self> {
        // check for .env extension
        assert!(
            file.extension().unwrap() == "env",
            "env file with missing extension: {}",
            file.display()
        );

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

    /// Returns a new Environment with `list` as it's variables.
    /// `internal_envs` will be empty.
    pub fn from_env_list(list: Vec<(String, String)>) -> Self {
        Environment {
            envs: list.into_iter().collect(),
            internal_envs: HashMap::new(),
        }
    }

    /// Loads and returns all currently loaded environment variables, complete with variables
    /// defined in `env_file`.
    ///
    /// If a variable set in `env_file` is already loaded, it will be overwritten with
    /// the value given in `env_file`.
    ///
    /// Does not set `internal_envs`.
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
    /// let envs = Environment::from_file_with_load(&mock_env_file).unwrap();
    ///
    /// // from_file_with_load returns **all** currently loaded envs, so there will be more than
    /// // just the one we set
    /// assert!(envs.to_env_map().len() > 1);
    ///
    /// // from_file_with_load has created a variable called "TEST" with the value "true"
    /// assert!(envs.contains_env_var("TEST"));
    /// assert_eq!(envs.get_env_val("TEST"), Some(&String::from("true")));
    ///
    /// // and it is actually loaded
    /// assert_eq!(dotenvy::var("TEST").unwrap(), "true");
    /// ```
    pub fn from_file_with_load(env_file: &PathBuf) -> Result<Self> {
        dotenvy::from_path_override(env_file)?;
        Ok(Environment::from_env_list(dotenvy::vars().collect()))
    }

    /// Serialize current envs to `file_path`.
    ///
    /// Will create a new file if `file_path` does not exist and will overwrite it if it does.
    /// This will fail if any parent directories of `file_path` do not exist.
    ///
    /// ## Errors
    /// - Returns an `EnvError` if writing failed
    pub fn to_file(&self, file_path: &PathBuf) -> Result<()> {
        let mut all = self.envs.clone();
        all.extend(self.internal_envs.clone());

        serde_envfile::to_file(file_path, &all).map_err(|e| Error::EnvError {
            reason: e.to_string(),
        })
    }

    /// Returns a map of all envs saved in this Environment.
    /// Does not include `internal_envs`
    pub fn to_env_map(&self) -> &HashMap<String, String> {
        &self.envs
    }

    /// Fills internal variables with their respective values.
    ///
    /// Overwrites `internal_envs` with the given values
    ///
    /// ## Parameters
    /// - `exp_src_dir` -> value of `$EXP_SRC_DIR`
    pub fn insert_internals(&mut self, exp_src_dir: &Path) {
        self.internal_envs.clear();

        self.internal_envs.insert(
            String::from("EXP_SRC_DIR"),
            exp_src_dir.canonicalize().unwrap().display().to_string(),
        );
    }

    /// Returns a map of all internal environments saved in this Environment
    pub fn get_internals(&self) -> &HashMap<String, String> {
        &self.internal_envs
    }

    /// Returns `true` if the variable exists in this Environment. Internal envs
    /// are not checked.
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
    ///
    /// Internal envs will not be changed
    pub fn extend_envs(&mut self, other_env: &Environment) {
        self.envs.extend(other_env.to_env_map().to_owned());
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
