use crate::helper::errors::Result;
use std::path::PathBuf;

pub trait CsvWriter {
    fn to_csv(&self, file: &PathBuf) -> Result<()>;
}

pub trait FileWriter {
    fn persist(&self, dir: &PathBuf) -> Result<()>;
}

pub trait FileReader {
    type Item;

    fn parse(dir: &PathBuf) -> Result<Self::Item>;
}
