use exomat::helper::{errors::Error, fs_names::*};
use indicatif::MultiProgress;
use std::path::PathBuf;

use crate::Result;

pub fn main(
    experiment: PathBuf,
    trial: Option<PathBuf>,
    output: Option<PathBuf>,
    repetitions: u64,
    log_handler: MultiProgress,
) -> Result<()> {
    let experiment = experiment.canonicalize()?;
    if experiment == std::env::current_dir()? {
        return Err(Error::HarnessRunError {
            experiment: file_name_string(&experiment.canonicalize()?),
            err: "Cannot start experiment run from pwd.".to_string(),
        });
    }

    if let Some(env) = trial {
        exomat::harness::run::trial(&experiment, env, log_handler)
    } else {
        let output = match output {
            Some(x) => Ok(x),
            None => exomat::harness::skeleton::generate_build_series_filepath(&experiment),
        }?;

        exomat::harness::run::experiment(&experiment, repetitions, output, log_handler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exomat::harness::skeleton;

    use rusty_fork::rusty_fork_test;
    use tempfile::TempDir;

    rusty_fork_test! {
        #[test]
        fn test_run() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();

            // create source
            let experiment = tmpdir.join("experiment");
            skeleton::main(&experiment).unwrap();

            // run
            let output = tmpdir.join("output");
            assert!(main(
                experiment,             // run this experiment
                None,                   // no trial
                Some(output.clone()),   // output to this path
                1,                      // one repetition
                MultiProgress::new(),   // log handler (unimportant for this test)
            ).is_ok());

            assert!(&output.is_dir());
        }

        // working trial run is tested in harness::run::trial_e2e()
        // testing this again here causes the same trial directory to be used, a.k.a.
        // the test would either need to sleep 1s or it will always fail
        // ... so we don't test it again

        #[test]
        fn test_trial_invalid_env() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();

            // create source
            let exp = tmpdir.join("experiment");
            skeleton::main(&exp).unwrap();

            // run with invalid trial env
            let trial_env = tmpdir.join("invalid");
            assert!(!trial_env.is_file());

            let res = main(
                exp.clone(),            // run this experiment
                Some(trial_env),        // trial with invalid env
                None,                   // output to this path
                1,                      // one repetition
                MultiProgress::new(),   // log handler (unimportant for this test)
            );

            assert!(res.is_err());

            // check for correct error
            if let Err(Error::EnvError { reason }) = res {
                assert!(reason.contains("env file with missing extension:"));
            } else {
                panic!("Expected HarnessRunError, got {res:?}");
            }
        }

        #[test]
        fn test_run_pwd() {
            let tmpdir = TempDir::new().unwrap();
            let tmpdir = tmpdir.path().to_path_buf();

            // create source
            let exp = tmpdir.join("experiment");
            skeleton::main(&exp).unwrap();
            std::env::set_current_dir(&exp).unwrap();

            // start run from pwd while it is not an experiment source
            let res = main(
                std::env::current_dir().unwrap(),
                None,
                None,
                1,
                MultiProgress::new(),
            );
            assert!(res.is_err());

            // check for correct error
            if let Err(Error::HarnessRunError { experiment: _, err }) = res {
                assert!(
                    err.contains("Cannot start experiment run from pwd"), "got {err:?}"
                );
            } else {
                panic!("Expected HarnessRunError, got {res:?}");
            }
        }
    }
}
