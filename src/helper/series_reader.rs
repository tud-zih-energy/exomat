use crate::harness::env::{EnvList, Environment};
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::*;

use csv::Writer;
use log::{error, warn};
use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

type OutList = HashMap<String, Vec<String>>;
type Observation = HashMap<String, String>;

#[derive(Clone, Debug)]
struct RunReader {
    env: Environment,
    out_files: Option<EnvList>,
}

#[derive(Debug)]
struct RunReaderIter<'a> {
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

impl RunReader {
    /// Use slice iterator for immutable iteration
    pub fn iter(&self) -> RunReaderIter {
        RunReaderIter {
            run_reader: self,
            index: 0,
        }
    }

    fn parse(run: &PathBuf) -> Result<Self> {
        // read env file
        let env = Environment::from_file(&run.join(RUN_ENV_FILE))?;

        // read out files
        let mut out: EnvList = HashMap::new();
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

    fn from_env_list(envlist: &EnvList) -> Self {
        RunReader {
            env: Environment::new(),
            out_files: Some(envlist.clone()),
        }
    }

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
#[derive(Debug)]
struct SeriesReader {
    runs: Vec<RunReader>,
    stdout_log: Option<String>,
    stderr_log: Option<String>,
    exomat_log: Option<String>,
}

struct SeriesReaderIter<'a> {
    series_reader: &'a SeriesReader,
    index: usize,
}

impl<'a> Iterator for SeriesReaderIter<'a> {
    type Item = RunReader;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.series_reader.runs.len() {
            let run = self.series_reader.runs[self.index].clone();
            self.index += 1;

            Some(run)
        } else {
            None
        }
    }
}
impl<'a> IntoIterator for &'a SeriesReader {
    type Item = RunReader;
    type IntoIter = SeriesReaderIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl SeriesReader {
    /// Use slice iterator for immutable iteration
    pub fn iter(&self) -> SeriesReaderIter {
        SeriesReaderIter {
            series_reader: self,
            index: 0,
        }
    }

    pub fn parse(series: &PathBuf) -> Self {
        // find all run dirs
        let runs: Vec<RunReader> = find_run_repetitions(&series.join(SERIES_RUNS_DIR))
            .iter()
            .filter_map(|run| {
                let r = RunReader::parse(run);
                if r.is_err() {
                    error!("Cannot parse run: {}", run.display());
                }

                r.ok()
            })
            .collect();

        // read log files
        let stdout_log = Self::read_log(&series.join(SERIES_RUNS_DIR).join(SERIES_STDOUT_LOG));
        let stderr_log = Self::read_log(&series.join(SERIES_RUNS_DIR).join(SERIES_STDERR_LOG));
        let exomat_log = Self::read_log(&series.join(SERIES_RUNS_DIR).join(SERIES_EXOMAT_LOG));

        SeriesReader {
            runs,
            stdout_log,
            stderr_log,
            exomat_log,
        }
    }

    /// Serializes it's content into `file`.
    ///
    /// If the no runs are found or all runs are empty, `file` will still be created.
    ///
    /// Uses the default CSV delimiter `,`. Any values containing it will be escaped using
    /// `""`.
    ///
    /// ## Errors
    /// - Returns a `CsvError` if something went wrong during the csv serialization
    pub fn to_csv(&self, file: &PathBuf) -> Result<()> {
        let mut wtr = Writer::from_path(file).map_err(|e| Error::CsvError {
            reason: e.to_string(),
        })?;

        if !self.runs_are_empty() {
            // turn self.runs into csv rows (contains header)
            let content = self.to_csv_rows();

            for row in content {
                wtr.write_record(row).map_err(|e| Error::CsvError {
                    reason: e.to_string(),
                })?;
            }
        }

        wtr.flush().map_err(|e| Error::CsvError {
            reason: e.to_string(),
        })
    }

    pub fn run_count(&self) -> usize {
        self.runs.len()
    }

    pub fn keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .runs
            .iter()
            .filter_map(|run| run.out_files.as_ref())
            .flat_map(|out| out.keys().map(|k| k.as_str()))
            .collect();

        // remove duplicate keys
        keys.sort();
        keys.dedup();
        keys
    }

    fn to_csv_rows(&self) -> Vec<Vec<String>> {
        // collect all rows as HashMap
        let mut rows: OutList = HashMap::new();
        for run in &self.runs {
            if let Some(out) = &run.out_files {
                rows.extend(out.clone())
            } else {
                rows.extend(HashMap::new())
            }
        }

        // collect all header
        let mut rows_vec: Vec<Vec<String>> =
            vec![self.keys().iter().map(|k| k.to_string()).collect()];

        // turn all data into one list
        let val_len = rows.values().map(|v| v.len()).max().unwrap_or(0);

        for i in 0..val_len {
            // (one entry = every ith element of each key)
            let mut row: Vec<String> = Vec::new();

            for key in self.keys() {
                let value = rows.get(key).unwrap();
                row.push(value.get(i).cloned().unwrap_or_else(|| String::new()));
            }

            rows_vec.push(row);
        }

        rows_vec
    }

    /// Checks if there is anything recorded in self.runs
    ///
    /// Returns `true` if:
    /// - there are no runs
    /// - there are runs, but none contain out_ files
    /// - there are runs with out_ files, but all out_ files are empty
    fn runs_are_empty(&self) -> bool {
        if self.runs.is_empty()
            || self.runs.iter().all(|run| run.out_files.is_none())
            || self
                .runs
                .iter()
                .all(|run| run.out_files.iter().all(|out| out.is_empty()))
        {
            true
        } else {
            false
        }
    }

    fn from_env_lists(list_of_envlist: Vec<EnvList>) -> Self {
        let runs: Vec<RunReader> = list_of_envlist
            .iter()
            .map(|envlist| RunReader::from_env_list(&envlist))
            .collect();

        SeriesReader {
            runs: runs,
            stdout_log: None,
            stderr_log: None,
            exomat_log: None,
        }
    }

    /// helper, that returns the content of a file if it is readable.
    /// Otherwise returns `None`
    fn read_log(path: &PathBuf) -> Option<String> {
        match read_to_string(path) {
            Ok(log) => Some(log),
            Err(_) => None,
        }
    }
}

fn find_run_repetitions(runs_dir: &Path) -> Vec<PathBuf> {
    let mut repetitions = Vec::<PathBuf>::new();

    // return the empty vector if runs_dir does not exist
    if !runs_dir.is_dir() {
        println!("runs dir empty");
        return repetitions;
    }

    for entry in runs_dir.read_dir().expect("Could not read dir") {
        if entry
            .as_ref()
            .expect("Entry not readable")
            .metadata()
            .expect("Metadata of entry not readable")
            .is_dir()
        {
            // if directory name starts with "run_", it is considered a run repetition
            if entry
                .as_ref()
                .unwrap()
                .path() // complete path
                .file_name() // last part of path; directory name
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("run_")
            {
                println!("found run: {}", entry.as_ref().unwrap().path().display());
                repetitions.push(entry.unwrap().path());
            }
        }
    }

    repetitions
}

fn find_all_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::<PathBuf>::new();

    for entry in dir.read_dir().expect("Could not read dir") {
        if entry
            .as_ref()
            .expect("Entry not readable")
            .metadata()
            .expect("Metadata of entry not readable")
            .is_file()
        {
            files.push(entry.unwrap().path());
        }
    }

    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    // Helper to setup a fake run directory with env and out files
    fn setup_series_dir() -> TempDir {
        let tmp_run = TempDir::new().unwrap();
        let runs_dir = tmp_run.path().to_path_buf();

        // create run rep
        let equal_run = runs_dir.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0);
        let unequal_run = runs_dir.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR1);
        let empty_run = runs_dir.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR2);
        std::fs::create_dir_all(&equal_run).unwrap();
        std::fs::create_dir_all(&unequal_run).unwrap();
        std::fs::create_dir_all(&empty_run).unwrap();

        // Create env file for all runs
        std::fs::write(&unequal_run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();
        std::fs::write(&equal_run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();
        std::fs::write(&empty_run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

        // Create out_ files (equal)
        std::fs::write(&equal_run.join("out_number"), "1\n2").unwrap();
        std::fs::write(&equal_run.join("out_word"), "one\ntwo").unwrap();

        // Create out_ files (unequal)
        std::fs::write(&unequal_run.join("out_number"), "1\n2\n3").unwrap();
        std::fs::write(&unequal_run.join("out_word"), "NA").unwrap();

        // Create out_ files (empty)
        std::fs::write(&unequal_run.join("out_number"), "1\n2\n").unwrap();
        std::fs::File::create(&unequal_run.join("out_word")).unwrap();

        tmp_run
    }

    fn setup_series_no_runs() -> TempDir {
        let tmp_run = TempDir::new().unwrap();
        let series = tmp_run.path().to_path_buf();
        let run = series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0);

        // Create env file
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

        tmp_run
    }

    fn setup_series_empty_runs() -> TempDir {
        let tmp_run = TempDir::new().unwrap();
        let series = tmp_run.path().to_path_buf();
        let run = series.join(SERIES_RUNS_DIR).join(TEST_RUN_REP_DIR0);

        // Create env file
        std::fs::create_dir_all(&run).unwrap();
        std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

        // create empty out files
        std::fs::File::create(&run.join("out_empty")).unwrap();

        tmp_run
    }

    fn setup_run_dir_shadow() -> TempDir {
        let tmp_run = TempDir::new().unwrap();
        let run = tmp_run.path().to_path_buf();

        // Create env file for both runs
        std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

        // Create out_ files (equal)
        std::fs::write(&run.join("out_VAR1"), "1").unwrap();
        std::fs::write(&run.join("out_word"), "one").unwrap();

        tmp_run
    }

    fn setup_run_dir() -> TempDir {
        let tmp_run = TempDir::new().unwrap();
        let run = tmp_run.path().to_path_buf();

        // Create env file for both runs
        std::fs::write(&run.join(RUN_ENV_FILE), "VAR1=foo\nVAR2=bar").unwrap();

        // Create out_ files (equal)
        std::fs::write(&run.join("out_number"), "1\n2").unwrap();
        std::fs::write(&run.join("out_word"), "one\ntwo").unwrap();

        tmp_run
    }

    #[test]
    fn seriesreader_iter() {
        // test iterating without error
        let tmp_run = setup_series_dir();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&runs_dir);
        assert_eq!(series_reader.run_count(), 3);

        // iterate over runs and observations
        for run in series_reader.iter() {
            println!("run: {run:?}");
            for obs in run.iter() {
                assert!(obs.get("number").is_some());
                assert!(obs.get("word").is_some());
            }
        }
    }

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
        let tmp = setup_series_no_runs();
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

    #[test]
    fn seriesreader_keys() {
        let tmp_run = setup_series_dir();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&runs_dir);
        let keys = series_reader.keys();

        assert!(keys.contains(&"number"));
        assert!(keys.contains(&"word"));
        assert!(keys.len() == 2);
    }

    #[test]
    fn seriesreader_keys_no_content() {
        let tmp_run = setup_series_empty_runs();
        let runs_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&runs_dir);
        let keys = series_reader.keys();

        assert!(keys.contains(&"empty"));
        assert!(keys.len() == 1);
    }

    #[test]
    fn seriesreader_keys_no_out_files() {
        let tmp_run = setup_series_no_runs();
        let series_dir = tmp_run.path().to_path_buf();

        let series_reader = SeriesReader::parse(&series_dir);
        let keys = series_reader.keys();
        assert!(keys.is_empty());
    }

    use crate::helper::test_fixtures::{
        envlist_1a, envlist_empty_string, envlist_mixed_weird, envlist_one_var_no_val,
        filled_series_run_duplicate, filled_series_run_invalid, filled_series_run_na,
        skeleton_series_run, skeleton_series_run_empty, skeleton_src,
    };
    use crate::helper::test_helper::contains_either;
    use rstest::rstest;

    #[rstest]
    fn reader_serialize_multiline(
        #[from(skeleton_src)] outdir: TempDir,
        envlist_mixed_weird: EnvList,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("2.csv");

        // not created yet
        assert!(!out_file.is_file());

        let reader = SeriesReader::from_env_lists(vec![envlist_mixed_weird]);
        reader.to_csv(&out_file).unwrap();

        // with multiple keys and values the order of items after serialization is
        // random, so only check if the correct lines are there
        let file_2 = std::fs::read_to_string(out_file).unwrap();
        assert!(contains_either(&file_2, "VAR1,VAR2\n", "VAR2,VAR1\n"));
        assert!(contains_either(&file_2, "VALUE,\n", ",VALUE\n"));
        assert!(contains_either(&file_2, "\"a,b\",baz\n", "baz,\"a,b\"\n"));
    }

    #[rstest]
    #[case(HashMap::new(), "")]
    #[case(envlist_1a(), "1\na\n")]
    #[case(envlist_one_var_no_val(), "VAR\n")]
    #[case(envlist_empty_string(), "VAR\n\"\"\n")]
    fn reader_serialize_single(
        #[from(skeleton_src)] outdir: TempDir,
        #[case] envlist: EnvList,
        #[case] expected: String,
    ) {
        let outdir = outdir.path().to_path_buf();
        let out_file = outdir.join("0.csv");

        // not created yet
        assert!(!out_file.is_file());

        let reader = SeriesReader::from_env_lists(vec![envlist]);
        reader.to_csv(&out_file).unwrap();

        assert_eq!(std::fs::read_to_string(out_file).unwrap(), expected);
    }
}
