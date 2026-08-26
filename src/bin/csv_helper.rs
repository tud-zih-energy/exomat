use super::cli_structure::CsvToFormat;
use csv::StringRecord;
use exomat::harness::env::Environment;
use exomat::helper::errors::{Error, Result};
use exomat::helper::fs_names::{MARKER_SRC, SRC_ENV_DIR};
use log::{debug, info, warn};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

/// info used when writing env files
struct CachedInfoEnv {
    env_dir: PathBuf,
    random_string: String,
}

impl CachedInfoEnv {
    pub fn build() -> Result<Self> {
        use exomat::helper::archivist::find_marker_pwd;
        use rand::distr::Alphanumeric;
        use rand::RngExt;

        let exp_source = find_marker_pwd(MARKER_SRC)?;
        let env_dir = exp_source.join(SRC_ENV_DIR);

        let random_string: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(6)
            .map(char::from)
            .collect();

        Ok(Self {
            env_dir,
            random_string,
        })
    }

    pub fn maybe_rm_default_env(&self) -> Result<()> {
        let path_default_env = self.env_dir.join("0.env");
        if let Ok(default_env_content) = std::fs::read_to_string(&path_default_env) {
            if default_env_content.is_empty() {
                info!("rm default-created env");
                std::fs::remove_file(&path_default_env)?;
            }
        }

        Ok(())
    }
}

fn check_header(header: &StringRecord, format: CsvToFormat) -> Result<()> {
    if header.is_empty() {
        return Err(Error::Empty("file must not be empty".to_string()));
    }

    for (col_index, col_name) in header.iter().enumerate() {
        if col_name.is_empty() {
            return Err(Error::Empty(format!(
                "header: name of column {col_index} may not be empty"
            )));
        }

        if col_name.contains("/") || col_name.contains("\n") {
            return Err(Error::CsvError{reason: format!("name of column {col_index} is malfored: may contain neither slash (/) nor newline")});
        }

        if format == CsvToFormat::Out
            && !col_name
                .chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            warn!("column name contains non-default chars for filenames (alphanum-_.): {col_name}");
        }

        if format == CsvToFormat::Env
            && !col_name
                .chars()
                .all(|c| c.is_uppercase() || c.is_numeric() || c == '_')
        {
            return Err(Error::EnvError{ reason: format!("column name contains non-default chars for variables (uppercase or numeric or _): {col_name}") });
        }
    }

    Ok(())
}

fn process_record(
    header: &StringRecord,
    record: &StringRecord,
    record_index: usize,
    format: CsvToFormat,
    cached_info_env: Option<&CachedInfoEnv>,
) -> Result<()> {
    match format {
        CsvToFormat::Out => process_record_format_out(header, record),
        CsvToFormat::Env => process_record_format_env(
            header,
            record,
            record_index,
            cached_info_env.ok_or(Error::Empty("CachedInfoEnv".to_string()))?,
        ),
    }
}

fn process_record_format_out(header: &StringRecord, record: &StringRecord) -> Result<()> {
    for (col_value, col_name) in record.iter().zip(header.iter()) {
        use std::io::Write;
        let mut out_file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(format!("out_{col_name}"))?;
        out_file.write_all(format!("{col_value}\n").as_bytes())?;
    }

    Ok(())
}

fn process_record_format_env(
    header: &StringRecord,
    record: &StringRecord,
    record_index: usize,
    cached_info_env: &CachedInfoEnv,
) -> Result<()> {
    let mut env = Environment::new();

    for (col_value, col_name) in record.iter().zip(header.iter()) {
        env.add_env(col_name.to_string(), col_value.to_string());
    }

    let fname = format!(
        "csv-{random}-{record_index:05}.env",
        random = cached_info_env.random_string
    );

    env.to_file(&cached_info_env.env_dir.join(fname))?;
    Ok(())
}

pub fn main(input: &Path, format: CsvToFormat, no_rm_default_env: bool) -> Result<()> {
    let mut reader = csv::Reader::from_path(input).map_err(|err| Error::CsvError {
        reason: format!("could not create csv reader for {}: {err}", input.display()),
    })?;

    let header = reader
        .headers()
        .map_err(|err| Error::CsvError {
            reason: format!("failed to read header: {err}"),
        })?
        .clone();

    check_header(&header, format)?;

    // init extra info
    let cached_info_env = match format {
        CsvToFormat::Env => Some(CachedInfoEnv::build()?),
        _ => None,
    };

    // process rows
    let mut processed_records = 0;
    for (line, record) in reader.records().enumerate() {
        let record = record.map_err(|err| Error::CsvError {
            reason: format!("csv reader error on line {line}: {err}"),
        })?;

        if record.is_empty() {
            return Err(Error::Empty(format!(
                "line {line}: records may not be empty"
            )));
        }

        if header.len() != record.len() {
            return Err(Error::CsvError {reason: format!("line {line}: incorrect number of columns, expected {header_num_cols} columns, but found {record_num_cols}", header_num_cols = header.len(), record_num_cols = record.len())});
        }

        process_record(&header, &record, line, format, cached_info_env.as_ref())?;
        processed_records += 1;
    }

    if 0 == processed_records {
        return Err(Error::Empty(format!(
            "csv must contain at least one row (beyond header): {}",
            input.display()
        )));
    }

    info!("processed {processed_records} records");

    if format == CsvToFormat::Env && !no_rm_default_env {
        debug!("trying to remove default env");
        cached_info_env
            .ok_or(Error::Empty("CachedInfoEnv".to_string()))?
            .maybe_rm_default_env()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_fixtures::skeleton_default_env;
    use exomat::helper::fs_names::SRC_ENV_DIR;
    use rusty_fork::rusty_fork_test;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    fn helper_get_vars(path: std::path::PathBuf) -> Vec<String> {
        let content = std::fs::read_to_string(&path).unwrap();
        content
            .strip_suffix("\n")
            .unwrap_or(&content)
            .split("\n")
            .map(|s| s.to_string())
            .collect()
    }

    fn minimal_env_csv_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "FOO").unwrap();
        writeln!(file, "42").unwrap();
        file
    }

    fn csv_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "foo,bar").unwrap();
        writeln!(file, "1,2").unwrap();
        writeln!(file, "3,4").unwrap();
        // note: no trailing newline, just for funsies
        write!(file, "17,42").unwrap();
        file
    }

    fn csv_env_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "FOO,BAR").unwrap();
        writeln!(file, "1,2").unwrap();
        writeln!(file, "3,4").unwrap();
        // note: no trailing newline, just for funsies
        write!(file, "17,42").unwrap();
        file
    }

    #[test]
    fn header_all_uppercase_env() {
        let header_upper = StringRecord::from_iter(vec!["ABC", "DEF_", "f00"]);

        check_header(&header_upper, CsvToFormat::Out).unwrap();
        let err = check_header(&header_upper, CsvToFormat::Env).unwrap_err();
        assert!(matches!(err, Error::EnvError{reason} if reason.contains("upper")));

        let header_valid = StringRecord::from_iter(vec!["ABC", "DEF_", "F00"]);
        check_header(&header_valid, CsvToFormat::Env).unwrap();
    }

    rusty_fork_test! {
        #[test]
        fn csv_empty() {
            let empty_file = NamedTempFile::new().unwrap();
            let err = main(empty_file.path(), CsvToFormat::Out, false).unwrap_err();
            assert!(matches!(dbg!(err), Error::Empty(err) if err.contains("file")));
        }

        #[test]
        fn csv_only_header() {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, "foo,bar").unwrap();

            let err = main(file.path(), CsvToFormat::Out, false).unwrap_err();
            assert!(matches!(err, Error::Empty(s) if s.contains("row")));
        }

        #[test]
        fn csv_malformed() {
            // note: may start writing files (as error occurs later), hence use tempdir
            let tempdir = TempDir::new().unwrap();
            std::env::set_current_dir(tempdir.path()).unwrap();

            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, "foo,bar").unwrap();
            writeln!(file, "1,2").unwrap();
            writeln!(file, "1").unwrap();

            let err = main(file.path(), CsvToFormat::Out, false).unwrap_err();
            assert!(matches!(dbg!(err), Error::CsvError{..}));
        }

        #[test]
        fn csv_header_empty() {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, ",bar").unwrap();
            writeln!(file, "1,3").unwrap();

            let err = main(file.path(), CsvToFormat::Out, false).unwrap_err();
            assert!(matches!(err, Error::Empty(s) if s.contains("header")));
        }

        #[test]
        fn simple_scenario() {
            let file = csv_file();

            let tempdir = TempDir::new().unwrap();
            std::env::set_current_dir(tempdir.path()).unwrap();

            main(file.path(), CsvToFormat::Out, false).unwrap();

            assert_eq!(vec!["1", "3", "17"], helper_get_vars(tempdir.path().join("out_foo")));
            assert_eq!(vec!["2", "4", "42"], helper_get_vars(tempdir.path().join("out_bar")));
        }

        #[test]
        fn trailing_newline_ok() {
            let mut file = NamedTempFile::new().unwrap();
            writeln!(file, "foo").unwrap();
            writeln!(file, "2").unwrap();

            let tempdir = TempDir::new().unwrap();
            std::env::set_current_dir(tempdir.path()).unwrap();

            main(file.path(), CsvToFormat::Out, false).unwrap();

            assert_eq!(vec!["2"], helper_get_vars(std::path::PathBuf::from("out_foo")));
        }

        #[test]
        fn default_env_kept() {
            let source_dir = skeleton_default_env();
            std::env::set_current_dir(source_dir.path()).unwrap();

            let csv = minimal_env_csv_file();
            main(csv.path(), CsvToFormat::Env, true).unwrap();

            let default_env = std::fs::read_to_string(source_dir.path().join(SRC_ENV_DIR).join("0.env")).unwrap();
            assert!(default_env.trim().is_empty());
        }

        #[test]
        fn non_default_env_kept() {
            let source_dir = skeleton_default_env();
            std::env::set_current_dir(source_dir.path()).unwrap();
            let env_dir = source_dir.path().join(SRC_ENV_DIR);

            let sentinel_content = "hjahb12n3mb";

            std::fs::write(env_dir.join("0.env"), &sentinel_content).unwrap();
            std::fs::write(env_dir.join("1.env"), "").unwrap();

            let csv = minimal_env_csv_file();
            main(csv.path(), CsvToFormat::Env, false).unwrap();


            assert_eq!(sentinel_content, std::fs::read_to_string(env_dir.join("0.env")).unwrap());
            assert_eq!("", std::fs::read_to_string(env_dir.join("1.env")).unwrap());
        }

        #[test]
        fn generate_env() {
            use std::collections::HashMap;

            let csv = csv_env_file();

            let source_dir = skeleton_default_env();
            std::env::set_current_dir(source_dir.path()).unwrap();
            let env_dir = source_dir.path().join(SRC_ENV_DIR);

            main(csv.path(), CsvToFormat::Env, false).unwrap();

            let all_envs_by_fname = exomat::harness::env::get_existing_environments_by_fname(&env_dir).unwrap();
            let all_envs: Vec<Environment> = all_envs_by_fname.into_values().collect();

            fn helper_env_map(value_foo: &str, value_bar: &str) -> HashMap<String, String> {
                HashMap::from([
                    ("FOO".to_string(), value_foo.to_string()),
                    ("BAR".to_string(), value_bar.to_string())
                ])
            }

            assert_eq!(3, all_envs.len());
            // note: row order is preserved in filenames
            assert_eq!(all_envs[0].to_env_map(), &helper_env_map("1", "2"));
            assert_eq!(all_envs[1].to_env_map(), &helper_env_map("3", "4"));
            assert_eq!(all_envs[2].to_env_map(), &helper_env_map("17", "42"));

            // note: by extension, default env is now destroyed (as it should be)
        }
    }
}
