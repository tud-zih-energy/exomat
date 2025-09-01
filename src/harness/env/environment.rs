use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::helper::errors::{Error, Result};

/// Represents one environment file
#[derive(Debug, Clone, PartialEq)]
pub struct Environment {
    variables: HashMap<String, String>,
}

impl Environment {
    /// Constructs an empty Environment
    pub fn new() -> Self {
        Environment {
            variables: HashMap::new(),
        }
    }

    /// Constructs a new Environment with all variables and values from a file.
    /// Does not include process environemnt variables.
    ///
    /// ## Parameters
    /// `file` needs to be a valid env file, see Errors and Panics
    ///
    ///  ## Errors and Panics
    /// - Panics if `file` does not end in ".env"
    /// - Returns an `EnvError` if `file` isn't a valid .env file (this does not include having
    ///   the correct extension) or if an error occured during parsing.
    pub fn from_file(file: &Path) -> Result<Self> {
        // check for .env extension
        assert!(
            file.extension().unwrap() == "env",
            "env file with missing extension: {}",
            file.display()
        );

        let mut env = Environment {
            variables: HashMap::new(),
        };

        // Not using serde_envfile here, because it converts "VAR" to "var" :(
        for item in dotenvy::from_filename_iter(file)? {
            let (var, val) = item.map_err(|e| Error::EnvError {
                reason: e.to_string(),
            })?;

            env.variables.insert(var, val);
        }

        Ok(env)
    }

    pub fn from_env_list(list: Vec<(String, String)>) -> Self {
        Environment {
            variables: list.into_iter().collect(),
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
    /// let envs = Environment::from_file_with_load(&mock_env_file).unwrap();
    ///
    /// // load_envs returns **all** currently loaded envs, so there will be more than
    /// // just the one we set
    /// assert!(envs.to_env_list().len() > 1);
    ///
    /// // load_envs has created a variable called "TEST" with the value "true"
    /// assert!(envs.contains_variable("TEST"));
    /// assert_eq!(envs.get_value("TEST"), Some(&String::from("true")));
    ///
    /// // and it is actually loaded
    /// assert_eq!(dotenvy::var("TEST").unwrap(), "true");
    /// ```
    pub fn from_file_with_load(env_file: &PathBuf) -> Result<Self> {
        dotenvy::from_path_override(env_file)?;
        Ok(Environment::from_env_list(dotenvy::vars().collect()))
    }

    /// Serialize current envs to `exp_src_envs/file_name`.
    ///
    /// Will create a new file if `file_name` does not exist and will overwrite it if it does.
    /// This will fail if any parent directories of `exp_src_envs` do not exist.
    ///
    /// ## Errors
    /// - Returns an EnvError if writing failed
    pub fn to_file(&self, file_path: &PathBuf) -> Result<()> {
        serde_envfile::to_file(file_path, &self.variables).map_err(|e| Error::EnvError {
            reason: e.to_string(),
        })
    }

    pub fn to_env_list(&self) -> &HashMap<String, String> {
        &self.variables
    }

    pub fn contains_variable(&self, var: &str) -> bool {
        self.variables.contains_key(var)
    }

    pub fn add_variable(&mut self, var: String, val: String) {
        self.variables.insert(var, val);
    }

    pub fn extend_variables(&mut self, other_env: &Environment) {
        self.variables.extend(other_env.to_env_list().to_owned());
    }

    pub fn get_value(&self, var: &str) -> Option<&String> {
        self.variables.get(var)
    }

    pub fn get_variables(&self) -> Vec<&String> {
        self.variables.keys().collect()
    }
}
