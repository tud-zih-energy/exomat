use std::collections::HashMap;

/// One row of values across all out_ files in an Experiment Run
pub type Observation = HashMap<String, String>;

/// Maps out_ file names to their content (separated by newlines)
pub type OutList = HashMap<String, Vec<String>>;

