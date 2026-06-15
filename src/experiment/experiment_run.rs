use super::experiment_traits::{FileReader, FileWriter};
use crate::experiment::out_file::{Observation, OutFile, OutList};
use crate::harness::env::{Environment, ExomatEnvironment};

use crate::helper::{
    archivist::{create_harness_file, find_all_files},
    errors::{Error, Result},
    fs_names::*,
};

use log::warn;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub enum RunStatus {
    Unknown,
    Ready,
    Success,
    Fail(String),
}

/// Container for an Experiment Run
#[derive(Clone, Debug, PartialEq)]
pub struct ExperimentRun {
    run_sh: String,
    env: Environment,
    out_files: Option<OutList>,
    status: RunStatus,
}

impl ExperimentRun {
    pub fn new(run_sh: &str, environment: &Environment) -> Self {
        // assert that all exomat env vars are added
        assert!(ExomatEnvironment::RESERVED_ENV_VARS
            .iter()
            .all(|k| environment.contains_env_var(k)));

        Self {
            run_sh: run_sh.to_string(),
            env: environment.clone(),
            out_files: None,
            status: RunStatus::Ready,
        }
    }

    /// Immutable iteration
    pub fn iter<'a>(&'a self) -> ExperimentRunIter<'a> {
        ExperimentRunIter {
            run_reader: self,
            index: 0,
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.env
    }

    /// Returns the content of an out_ file `out_[var]`
    ///
    /// If there is no file with this name, `None` is returned.
    /// The returned Vec may be empty.
    pub fn get_var(&self, var: &str) -> Option<&Vec<String>> {
        match &self.out_files {
            Some(out) => {
                if let Some(outfile) = out.get_outfile(var) {
                    Some(outfile.values())
                } else {
                    None
                }
            }
            None => None,
        }
    }

    /// Returns a reference to the list of out_ files recorded
    pub fn get_out_files(&self) -> &Option<OutList> {
        &self.out_files
    }

    /// Replaces `self.out_files` with `new_out`.
    ///
    /// The content of `new_out` is not checked in any way.
    pub fn replace_out_files_unchecked(&mut self, new_out: Option<OutList>) {
        self.out_files = new_out
    }

    /// Generates a RunReader from `outlist`.
    ///
    /// Sets an empty Environemnt.
    pub fn from_out_list_unchecked(outlist: &OutList) -> Self {
        ExperimentRun {
            run_sh: String::new(),
            env: Environment::new(),
            out_files: Some(outlist.clone()),
            status: RunStatus::Ready,
        }
    }

    /// Helper, that returns the value at `index` for all keys in the current run.
    ///
    /// If a value is empty "NA" will be returned as the key's value.
    ///
    /// - Returns an `IndexOutOfRange` Error if the index is out of range (unbelievable, I know)
    /// - Returns an `Empty` Error if there are no Observations, that can be returned
    fn get_observation(&self, index: usize) -> Result<Observation> {
        // there are out_files
        if let Some(out_files) = &self.out_files {
            let mut observation: Observation = HashMap::new();
            for outfile in out_files.iter() {
                // no values recorded
                if outfile.is_empty() {
                    observation.insert(outfile.var_name().clone(), String::from("NA"));
                    // index is not in range
                } else if index >= outfile.value_count() {
                    return Err(Error::IndexOutOfRange {
                        index,
                        limit: outfile.value_count(),
                    });
                    // everything worked, get value
                } else {
                    observation.insert(
                        outfile.var_name().clone(),
                        outfile.values()[index].to_string(),
                    );
                }
            }

            Ok(observation)

        // no out_files in this run
        } else {
            Err(Error::Empty(String::from("No Observations found")))
        }
    }
}

// ========================== Reader ==========================
impl FileReader for ExperimentRun {
    type Item = ExperimentRun;

    /// Parses an Experiment Run into a RunReader object.
    ///
    /// Will balance out missing values, if possible, so that the number of values
    /// is even across all out_ files.
    ///
    /// The content of out_ files is not validated or checked in any way, if you put
    /// weird content in them, you will get weird output.
    ///
    /// ### Warnings, Errors and Panics
    /// What you will be **warn**ed about:
    /// - no env file at run/[RUN_ENV_FILE]
    /// - an out_ file shadows an env var
    ///
    /// What will cause an **Error**:
    /// - invalid out_ file names
    /// - unbalanced multiline out_ files
    ///
    /// This function my **Panic** if reading/writing failed.
    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        // read env file
        let env = Environment::from_file(&dir.join(RUN_ENV_FILE)).unwrap_or_else(|_| {
            warn!("No environment found in run {}", dir.display());
            Environment::new()
        });

        // read out_file
        let run_sh = std::fs::read_to_string(&dir.join(RUN_RUN_FILE))?;

        // read out files
        let mut out_list: OutList = OutList::default();
        let contained_files = find_all_files(&dir)?;

        for file in contained_files {
            match OutFile::parse(&file) {
                Err(Error::Empty(e)) => return Err(Error::Empty(e)), // this means the name is invalid
                Err(_) => continue,
                Ok(outfile) => {
                    // warn if out file shadows env var
                    if env.contains_env_var(outfile.var_name()) {
                        warn!(
                            "in {}: out_{} shadows input environment variable ${}",
                            outfile.var_name(),
                            dir.display(),
                            outfile.var_name(),
                        );
                    }

                    // extend existing outlist
                    if out_list.contains(&outfile) {
                        let to_extend = out_list
                            .iter_mut()
                            .find(|f| f.var_name() == outfile.var_name())
                            .expect("Could not locate out file to append to");

                        to_extend.extend_values(outfile.values());
                    } else {
                        out_list.push(outfile);
                    }
                }
            }
        }

        // balance values
        let out_balanced = match out_list.is_empty() {
            true => None,
            false => {
                let max_length = out_list
                    .iter()
                    .map(|out| out.value_count())
                    .max()
                    .unwrap_or(1);

                // for each variable
                for outfile in out_list.iter_mut() {
                    let len = outfile.value_count();

                    if len == 1 && max_length > 1 {
                        let to_extend = max_length - len;
                        outfile.repeat(0, to_extend);

                        // We got multiple values for var, check if it has the same number of rows as the
                        // other columns
                    } else if len != max_length {
                        return Err(Error::EnvError {
                                        reason: format!("Mismatched number of values for {} {len}, other value in {} has {max_length}", outfile.var_name(), dir.display())});
                    }
                }

                Some(out_list)
            }
        };

        Ok(ExperimentRun {
            run_sh,
            env: env,
            out_files: out_balanced,
            status: RunStatus::Unknown,
        })
    }
}

// ========================== Iterator ==========================
/// Iterator for RunReader
///
/// Iterates over the Observations in an EXperiment Run.
#[derive(Debug)]
pub struct ExperimentRunIter<'a> {
    run_reader: &'a ExperimentRun,
    index: usize,
}

impl<'a> Iterator for ExperimentRunIter<'a> {
    type Item = Observation;

    fn next(&mut self) -> Option<Self::Item> {
        let obs = self.run_reader.get_observation(self.index);
        self.index += 1;

        match obs {
            Ok(obs) => Some(obs),
            Err(_) => None,
        }
    }
}
impl<'a> IntoIterator for &'a ExperimentRun {
    type Item = Observation;
    type IntoIter = ExperimentRunIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ========================== Tests ==========================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::test_fixtures::{setup_run_dir, setup_run_dir_shadow, setup_series_no_out};

    #[test]
    fn runreader_iter_working() {
        let tmp_run = setup_run_dir();
        let runs_dir = tmp_run.path().to_path_buf();

        let run_reader = ExperimentRun::parse(&runs_dir).unwrap();
        let mut run_iter = run_reader.iter();

        // iterate over runs, should work
        let obs1 = run_iter.next().unwrap();
        assert_eq!(obs1.get("number").unwrap(), "1");
        assert_eq!(obs1.get("word").unwrap(), "one");

        let obs2 = run_iter.next().unwrap();
        assert_eq!(obs2["number"], "2");
        assert_eq!(obs2["word"], "two");

        assert!(run_iter.next().is_none());
    }

    #[test]
    fn runreader_empty_out_files() {
        let tmp = setup_series_no_out();
        let series = tmp.path().to_path_buf();

        // iterator is created, but no observations
        let run_reader =
            ExperimentRun::parse(&series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0)).unwrap();
        let mut run_iter = run_reader.iter();
        println!("{run_iter:?}");
        assert!(run_iter.next().is_none());
    }

    #[test]
    fn runreader_out_file_shadows_env_var() {
        let tmp = setup_run_dir_shadow();
        let run_dir = tmp.path().to_path_buf();

        // Should not panic, but log a warning
        let run_reader = ExperimentRun::parse(&run_dir).unwrap();
        let mut iter = run_reader.iter();

        let obs = iter.next().unwrap();
        assert_eq!(obs.get("VAR1").unwrap(), "1");
        assert_eq!(obs.get("word").unwrap(), "one");
    }
}
