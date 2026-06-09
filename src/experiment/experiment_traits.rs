pub trait CsvWriter {
    fn to_csv(&self, file: &PathBuf) -> Result<()>;
}

pub trait FileWriter {
    fn persist(&self, dir: &PathBuf) -> Result<()>;
}

pub trait FileReader {
    fn parse(&self, dir: &PathBuf) -> Result<()>;
}
