use crate::helper::errors::{Error, Result};

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

pub trait CsvWriter {
    fn to_csv(&self, file: &Path) -> Result<()>;
}

pub trait FileWriter {
    fn persist(&mut self, dir: &Path) -> Result<()>;

    /// Helper that creates a new file at `file_path`. The file will be executable.
    fn create_executable(&self, file_path: &PathBuf) -> Result<File> {
        OpenOptions::new()
            .mode(0o775)
            .write(true)
            .create_new(true)
            .open(file_path)
            .map_err(|e| Error::HarnessCreateError {
                entry: file_path.display().to_string(),
                reason: e.to_string(),
            })
    }

    //Helper to replace a file's content
    fn write_to_file(&self, file: &mut File, content: &[u8]) -> Result<()> {
        file.write_all(content)?;
        Ok(())
    }

    //Helper to append to the end of a file
    fn append_to_file(&self, file_path: &PathBuf, content: &str) -> Result<()> {
        OpenOptions::new()
            .append(true)
            .open(file_path)
            .map_err(|e| Error::HarnessCreateError {
                entry: file_path.display().to_string(),
                reason: e.to_string(),
            })?;

        std::fs::write(&file_path, content)?;
        Ok(())
    }
}

pub trait LogWriter {
    fn persist_logs(&mut self) -> Result<()>;
}

pub trait FileReader {
    type Item;

    fn parse(dir: &Path) -> Result<Self::Item>;
}

pub trait Runner {
    type Item;

    fn execute(&mut self, exp_name: &str) -> Result<Self::Item>;
}
