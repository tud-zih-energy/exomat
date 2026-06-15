use crate::helper::errors::Result;
use std::path::PathBuf;

pub trait CsvWriter {
    fn to_csv(&self, file: &PathBuf) -> Result<()>;
}

pub trait FileWriter {
    fn persist(&mut self, dir: &PathBuf) -> Result<()>;
}

pub trait FileReader {
    type Item;

    fn parse(dir: &PathBuf) -> Result<Self::Item>;
}

pub trait Runner {
    fn execute(&mut self, exp_name: String) -> Result<(String, String)>;
}
