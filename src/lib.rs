#![doc=include_str!("../README.md")]
#![doc=include_str!("../docs/glossary.md")]
#![doc=include_str!("../docs/control_flow.md")]
#![doc=include_str!("../docs/build.md")]

pub mod harness {
    pub mod env;
    pub mod run;
    pub mod skeleton;
    pub mod table;
}

pub mod experiment;

pub mod helper {
    pub mod archivist;
    pub mod errors;
    pub mod fs_names;
    pub mod logging;
}

#[cfg(test)]
pub mod test_fixtures;

#[cfg(test)]
pub mod test_helper;

use helper::archivist::find_marker_pwd;
use helper::fs_names::*;
