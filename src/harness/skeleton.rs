//! harness skeleton subcommand

use std::path::Path;

use crate::experiment::{ExperimentSource, FileWriter};
use crate::helper::errors::Result;

/// entrypoint for skeleton binary
pub fn main(exp_src_dir: &Path) -> Result<()> {
    let mut src = ExperimentSource::new();
    src.persist(exp_src_dir)?;

    println!(
        r#"
Next steps:

# add variables
exomat -C {dir} env COUNT 1 2 3

# adjust script
vim {dir}/template/run.sh

# execute experiment
exomat run {dir}
"#,
        dir = exp_src_dir.display()
    );

    Ok(())
}
