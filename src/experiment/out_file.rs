use std::collections::{HashMap, HashSet};
use std::fmt::Display;
use std::fs::read_to_string;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;

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

    pub fn max_length(&self) -> usize {
        self.out_files
            .iter()
            .map(|out| out.value_count())
            .max()
            .unwrap_or(0)
    }

    pub fn get_outfile(&self, var_name: &str) -> Option<&OutFile> {
        self.out_files
            .iter()
            .find(|outfile| outfile.var_name() == var_name)
    }

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

#[derive(PartialEq, Clone, Debug)]
pub struct OutFile {
    name: String,
    content: Vec<String>,
}

impl OutFile {
    pub fn from(name: &str, content: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            content,
        }
    }

    pub fn var_name(&self) -> &String {
        &self.name
    }

    pub fn values(&self) -> &Vec<String> {
        &self.content
    }

    pub fn extend_values(&mut self, new_vals: &Vec<String>) {
        self.content.extend(new_vals.clone())
    }

    pub fn repeat(&mut self, index: usize, by: usize) {
        let value = self.content[index].clone();

        // Cannot use Vec::repeat() here, because String does not implement the Copy Trait >:(
        self.content.extend(vec![value; by]);
    }

    pub fn value_count(&self) -> usize {
        self.content.len()
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

// ========================== Reader ==========================
impl FileReader for OutFile {
    type Item = OutFile;

    fn parse(dir: &PathBuf) -> Result<Self::Item> {
        if !dir.is_file() {
            return Err(Error::ReaderError {
                dir: dir.display().to_string(),
                reason: "Entry is not a file".to_string(),
            });
        }

        let prefix = "out_";
        let file_name = file_name_string(dir);

        if file_name.starts_with(prefix) {
            // parse variable name from out file
            let name = file_name.strip_prefix(prefix).unwrap().to_string();
            if name.is_empty() {
                return Err(Error::Empty(
                    "variable name (prefix out_ alone is not permitted)".to_string(),
                ));
            }

            // read content
            let values = read_to_string(dir)?
                .trim()
                .split("\n")
                .map(|v| v.to_string())
                .collect();

            Ok(Self {
                name: name,
                content: values,
            })
        } else {
            Err(Error::ReaderError {
                dir: dir.display().to_string(),
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
                "{}: {:?} (...) {} more items)",
                self.name,
                cut_content,
                self.value_count()
            )
        } else if self.is_empty() {
            write!(f, "{} is empty)", self.name)
        } else {
            write!(f, "{}: {:?}", self.name, self.content)
        }
    }
}
