use crate::harness::env::Environment;
use crate::harness::table::{Observation, OutList};
use crate::helper::archivist::find_all_files;
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

use log::warn;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;

/// Container for an Experiment Run
#[derive(Clone, Debug)]
pub struct RunReader {
    env: Environment,
    out_files: Option<OutList>,
}

impl RunReader {
    /// Immutable iteration
    pub fn iter(&self) -> RunReaderIter {
        RunReaderIter {
            run_reader: self,
            index: 0,
        }
    }

    /// Returns the content of an out_ file `out_[var]`
    ///
    /// If there is no file with this name, `None` is returned.
    /// The returned Vec may be empty.
    pub fn get_var(&self, var: &str) -> Option<&Vec<String>> {
        match &self.out_files {
            Some(out) => out.get(var),
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
    pub fn parse(run: &PathBuf) -> Result<Self> {
        // read env file
        let env = Environment::from_file(&run.join(RUN_ENV_FILE)).unwrap_or_else(|_| {
            warn!("No environment found in run {}", run.display());
            Environment::new()
        });

        // read out files
        let mut out: OutList = HashMap::new();
        let prefix = "out_";
        let contained_files = find_all_files(&run)?;
        for file in contained_files.iter().filter_map(|file| {
            file.file_name()
                .and_then(|name| name.to_str())
                .filter(|name| name.starts_with(prefix))
                .map(|_| file)
        }) {
            // parse variable name from out file
            let var_name = file_name_string(file)
                .strip_prefix(prefix)
                .unwrap()
                .to_string();
            if var_name.is_empty() {
                return Err(Error::Empty(
                    "variable name (prefix out_ alone is not permitted)".to_string(),
                ));
            }

            // warn if out file shadows env var
            if env.contains_env_var(&var_name) {
                warn!(
                    "in {}: out_{var_name} shadows input environment variable ${var_name}",
                    run.display()
                );
            }

            // read content
            let new_val = read_to_string(file)?
                .trim()
                .split("\n")
                .map(|v| v.to_string())
                .collect();

            if out.contains_key(&var_name) {
                let val = out
                    .get_mut(&var_name)
                    .expect("Could not update output list");
                val.extend(new_val);
            } else {
                out.insert(var_name, new_val);
            }
        }

        // balance values
        let out_balanced = match out.is_empty() {
            true => None,
            false => {
                let max_length = out.values().map(|value| value.len()).max().unwrap_or(1);

                // for each variable
                for (var, vals) in out.iter_mut() {
                    if vals.len() == 1 && max_length > 1 {
                        // Cannot use Vec::repeat() here, because String does not implement the Copy Trait >:(
                        let to_extend = max_length - vals.len();
                        vals.extend(vec![vals[0].clone(); to_extend]);

                        // We got multiple values for var, check if it has the same number of rows as the
                        // other columns
                    } else if vals.len() != max_length {
                        return Err(Error::EnvError {
                                        reason: format!("Mismatched number of values for {var} {}, other value in {} has {max_length}", vals.len(), run.display())});
                    }
                }

                Some(out)
            }
        };

        Ok(RunReader {
            env: env,
            out_files: out_balanced,
        })
    }

    /// Generates a RunReader from `envlist`.
    ///
    /// Sets an empty Environemnt.
    pub fn from_env_list_unchecked(envlist: &OutList) -> Self {
        RunReader {
            env: Environment::new(),
            out_files: Some(envlist.clone()),
        }
    }

    /// Helper, that returns the value at `index` for all keys in the current run.
    ///
    /// If a value is empty "NA" will be returned as the key's value.
    ///
    /// - Returns an `EnvError` if the index is out of range
    /// - Returns an `Empty` Error if there are no Observations, that can be returned
    fn get_observation(&self, index: usize) -> Result<Observation> {
        // there are out_files
        if let Some(out_files) = &self.out_files {
            let mut observation: Observation = HashMap::new();
            for (var, vals) in out_files {
                // no values recorded
                if vals.is_empty() {
                    observation.insert(var.to_string(), String::from("NA"));
                    // index is not in range
                } else if index >= vals.len() {
                    //TODO: error type
                    return Err(Error::EnvError {
                        reason: String::from("Index out of range"),
                    });
                    // everything worked, get value
                } else {
                    observation.insert(var.to_string(), vals[index].to_string());
                }
            }

            Ok(observation)

        // no out_files in this run
        } else {
            Err(Error::Empty(String::from("No Observations found")))
        }
    }
}

// ========================== Iterator ==========================
/// Iterator for RunReader
///
/// Iterates over the Observations in an EXperiment Run.
#[derive(Debug)]
pub struct RunReaderIter<'a> {
    run_reader: &'a RunReader,
    index: usize,
}

impl<'a> Iterator for RunReaderIter<'a> {
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
impl<'a> IntoIterator for &'a RunReader {
    type Item = Observation;
    type IntoIter = RunReaderIter<'a>;

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

        let run_reader = RunReader::parse(&runs_dir).unwrap();
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
            RunReader::parse(&series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0)).unwrap();
        let mut run_iter = run_reader.iter();
        println!("{run_iter:?}");
        assert!(run_iter.next().is_none());
    }

    #[test]
    fn runreader_out_file_shadows_env_var() {
        let tmp = setup_run_dir_shadow();
        let run_dir = tmp.path().to_path_buf();

        // Should not panic, but log a warning
        let run_reader = RunReader::parse(&run_dir).unwrap();
        let mut iter = run_reader.iter();

        let obs = iter.next().unwrap();
        assert_eq!(obs.get("VAR1").unwrap(), "1");
        assert_eq!(obs.get("word").unwrap(), "one");
    }
}
