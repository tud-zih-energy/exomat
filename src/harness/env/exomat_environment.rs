//! Container for internal exomat environment variables

use std::path::PathBuf;

use crate::harness::env::environment::Environment;
use crate::helper::errors::Result;

pub struct ExomatEnvironment {
    pub exp_src_dir: PathBuf,
    pub repetition: u64,
}

impl ExomatEnvironment {
    pub fn new(exp_src_dir: &PathBuf, repetition: u64) -> Self {
        ExomatEnvironment {
            exp_src_dir: exp_src_dir.to_owned(),
            repetition: repetition,
        }
    }

    /// Returns an Environment with all variables of the `ExomatEnvironment`. This means it contains:
    ///
    /// - "EXP_SRC_DIR" (absolute path)
    /// - "REPETITION"
    pub fn to_environment_full(&self) -> Environment {
        let mut env = self.to_environment_serializable();

        env.extend_envs(&Environment::from_env_list(Vec::from([(
            String::from("EXP_SRC_DIR"),
            self.exp_src_dir
                .canonicalize()
                .unwrap()
                .display()
                .to_string(),
        )])));

        env
    }

    /// Returns an Environment with all environment variables that are allowed to
    /// be serialized. This means it contains:
    ///
    /// - "REPETITION"
    pub fn to_environment_serializable(&self) -> Environment {
        Environment::from_env_list(Vec::from([(
            String::from("REPETITION"),
            self.repetition.to_string(),
        )]))
    }

    /// List of all environment variable names that exomat reserves for internal use
    pub const RESERVED_ENV_VARS: [&str; 2] = ["EXP_SRC_DIR", "REPETITION"];
}

/// Adds serializable exomat envs to an env file
///
/// 1. Reads the environment from `env_path`
/// 2. adds all envs from `exomat_environment.to_environment_serializable()`
/// 3. serializes this back into `env_path`
pub fn append_exomat_envs(
    env_path: &PathBuf,
    exomat_environment: &ExomatEnvironment,
) -> Result<()> {
    let mut old_env = Environment::from_file(&env_path)?;
    let to_add = exomat_environment.to_environment_serializable();

    old_env.extend_envs(&to_add);
    old_env.to_file(&env_path)
}
