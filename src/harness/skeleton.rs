//! harness skeleton subcommand

use std::path::Path;

use crate::experiment::{ExperimentSource, FileWriter};
use crate::helper::errors::Result;

/// entrypoint for skeleton binary
pub fn main(exp_src_dir: &Path) -> Result<()> {
    let mut src = ExperimentSource::new();
    src.persist(exp_src_dir)?;

    println!();
    println!("next steps:");
    println!("1. add variables with:");
    println!("   exomat env --add COUNT 1 2 3");
    println!("2. adjust script in template/run.sh");
    println!("3. execute experiment with:");
    println!("   exomat run {}", exp_src_dir.display());

    Ok(())
}
