use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::read_to_string;
use std::ops::{Deref, DerefMut};
use std::path::Path;

use super::FileReader;
use crate::helper::errors::{Error, Result};
use crate::helper::fs_names::file_name_string;

/// One row of values across all out_ files in an Experiment Run
pub type Observation = HashMap<String, String>;

/// Maps out_ file names to their content (separated by newlines)
#[derive(Clone, Debug, PartialEq, Default)]
pub struct OutList {
    out_files: Vec<OutFile>,
}

impl OutList {
    /// Creates a new OutList.
    ///
    /// Ensures all out_ file names are unique.
    ///
    /// ## Errors
    /// - Returns a `ReaderError` if duplicate out_ file names are found
    pub fn from(out_files: Vec<OutFile>) -> Result<Self> {
        // Ensure all outfile names are unique
        let mut names = HashSet::new();
        for outfile in &out_files {
            if !names.insert(outfile.var_name()) {
                return Err(Error::ReaderError {
                    dir: format!("out_{}", outfile.var_name()),
                    reason: "duplicate out file names are forbidden".to_string(),
                });
            }
        }

        Ok(Self { out_files })
    }

    /// Returns the length of the longest OutFile in out_files
    ///
    /// If the maximum length cannot be determined, 0 is returned.
    pub fn max_length(&self) -> usize {
        self.out_files
            .iter()
            .map(|out| out.value_count())
            .max()
            .unwrap_or(0)
    }

    /// Returns the out_ file, where out_file.name == var_name
    pub fn outfile(&self, var_name: &str) -> Option<&OutFile> {
        self.out_files
            .iter()
            .find(|outfile| outfile.var_name() == var_name)
    }

    /// Extends the list of out_ files
    pub fn extend_list(&mut self, new_list: &OutList) {
        self.out_files.extend(new_list.out_files.clone());
    }
}

impl Deref for OutList {
    type Target = Vec<OutFile>;

    fn deref(&self) -> &Self::Target {
        &self.out_files
    }
}

impl DerefMut for OutList {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.out_files
    }
}

/// Represents one out_ file in an Experiment Run
#[derive(PartialEq, Clone, Debug)]
pub struct OutFile {
    name: String,
    content: Vec<String>,
}

impl OutFile {
    /// Create a new out_ file
    pub fn from(name: &str, content: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            content,
        }
    }

    /// Returns the name of the out_ file
    pub fn var_name(&self) -> &String {
        &self.name
    }

    /// Returns the content of out_ files
    pub fn values(&self) -> &Vec<String> {
        &self.content
    }

    /// Extends the content of this out_ file.
    ///
    /// One String in new_vals should represent one line of an out_ file.
    /// It's not recommended to ignore this.
    pub fn extend_values(&mut self, new_vals: &[String]) {
        self.content.extend(new_vals.to_owned())
    }

    /// Copy the item at `index` `by` times at the end of the content list
    ///
    /// ## Errors
    /// - returns an `IndexOutOfRange` Error, if the index is out of range (who would have guessed)
    pub fn repeat(&mut self, index: usize, by: usize) -> Result<()> {
        if index >= self.content.len() {
            return Err(Error::IndexOutOfRange {
                index,
                limit: self.content.len(),
            });
        }

        // Cannot use Vec::repeat() here, because String does not implement the Copy Trait >:(
        let value = self.content[index].clone();
        self.content.extend(vec![value; by]);
        Ok(())
    }

    /// Returns the length of self.content
    pub fn value_count(&self) -> usize {
        self.content.len()
    }

    /// Convinience function to check if self.content is empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

// ========================== Reader ==========================
impl FileReader for OutFile {
    type Item = OutFile;

    /// Parses the content of outfile into an OutFile object.
    ///
    /// ## Errors
    /// - Returns a `ReaderError` if outfile is not a file
    /// - Returns a `ReaderError` if outfile does not start with "out_"
    /// - Returns an `Empty` Error if outfile has an invalid name
    fn parse(outfile: &Path) -> Result<Self::Item> {
        if !outfile.is_file() {
            return Err(Error::ReaderError {
                dir: outfile.display().to_string(),
                reason: "Entry is not a file".to_string(),
            });
        }

        let prefix = "out_";
        let file_name = file_name_string(outfile);

        if file_name.starts_with(prefix) {
            // parse variable name from out file
            let name = file_name.strip_prefix(prefix).unwrap().to_string();
            if name.is_empty() {
                return Err(Error::Empty(
                    "variable name (prefix out_ alone is not permitted)".to_string(),
                ));
            }

            // read content
            let content = read_to_string(outfile)?
                .trim()
                .split("\n")
                .map(|v| v.to_string())
                .collect();

            Ok(Self { name, content })
        } else {
            Err(Error::ReaderError {
                dir: outfile.display().to_string(),
                reason: "not an out file".to_string(),
            })
        }
    }
}

// ========================== Writer ==========================
impl Display for OutFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.value_count() > 5 {
            let (cut_content, _) = self.content.split_at(5);

            write!(
                f,
                "{}: {:?} (... {} more items)",
                self.name,
                cut_content,
                self.value_count() - 5
            )
        } else if self.is_empty() {
            write!(f, "{} is empty", self.name)
        } else {
            write!(f, "{}: {:?}", self.name, self.content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::test_helper::create_out_file;

    #[test]
    fn outfile_repeat() {
        let mut outfile = OutFile::from("broken", vec!["only".to_string()]);
        assert!(outfile.repeat(1, 1).is_err());
    }
    #[test]
    fn outfile_repeat_out_of_bounds() {
        let mut outfile = OutFile::from("broken", vec!["only".to_string()]);
        outfile.repeat(0, 3).unwrap();

        assert_eq!(
            outfile,
            OutFile::from(
                "broken",
                vec![
                    "only".to_string(),
                    "only".to_string(),
                    "only".to_string(),
                    "only".to_string()
                ]
            )
        );
    }

    #[test]
    fn outfile_display() {
        let outfile_empty = OutFile::from("nothing", Vec::new());
        let outfile_less = OutFile::from("one", vec!["value".to_string()]);
        let outfile_equal = OutFile::from("few", (0..5).map(|n| n.to_string()).collect());
        let outfile_more = OutFile::from("many", (0..7).map(|n| n.to_string()).collect());

        assert_eq!(outfile_empty.value_count(), 0);
        assert_eq!(outfile_less.value_count(), 1);
        assert_eq!(outfile_equal.value_count(), 5);
        assert_eq!(outfile_more.value_count(), 7);

        assert_eq!(format!("{outfile_empty}"), "nothing is empty");
        assert_eq!(format!("{outfile_less}"), "one: [\"value\"]");
        assert_eq!(
            format!("{outfile_equal}"),
            "few: [\"0\", \"1\", \"2\", \"3\", \"4\"]"
        );
        assert_eq!(
            format!("{outfile_more}"),
            "many: [\"0\", \"1\", \"2\", \"3\", \"4\"] (... 2 more items)"
        );
    }

    #[test]
    fn parse_outfile_success() {
        let tmpdir = tempfile::TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();
        let outfile = create_out_file(&tmpdir, None, "out_test", "line1\nline2");

        let parsed = OutFile::parse(&outfile).unwrap();
        assert_eq!(parsed.var_name(), "test");
        assert_eq!(
            parsed.values(),
            &vec!["line1".to_string(), "line2".to_string()]
        );
    }

    #[test]
    fn parse_outfile_not_out() {
        let tmpdir = tempfile::TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();
        let outfile = create_out_file(&tmpdir, None, "not_out", "line1");

        assert!(OutFile::parse(&tmpdir).is_err()); // cannot parse dir
        assert!(OutFile::parse(&outfile).is_err()); // cannot parse non-outfile
    }

    #[test]
    fn parse_outfile_empty_name() {
        let tmpdir = tempfile::TempDir::new().unwrap();
        let tmpdir = tmpdir.path().to_path_buf();
        let outfile = create_out_file(&tmpdir, None, "out_", "line1");

        assert!(OutFile::parse(&outfile).is_err());
    }

    #[test]
    fn outlist_from_outfiles_success() {
        let list = OutList::from(vec![
            OutFile::from("a", vec!["x".to_string()]),
            OutFile::from("b", vec!["y".to_string(), "z".to_string()]),
        ])
        .unwrap();

        assert_eq!(list.max_length(), 2);
        assert_eq!(list.len(), 2);
        assert_eq!(list.outfile("missing"), None);
        assert_eq!(
            list.outfile("a").unwrap(),
            &OutFile::from("a", vec!["x".to_string()])
        );
    }

    #[test]
    fn out_list_from_duplicate_name_error() {
        let a = OutFile::from("dup", vec!["x".to_string()]);
        let b = OutFile::from("dup", vec!["y".to_string()]);

        assert!(OutList::from(vec![a, b]).is_err());
    }
}
