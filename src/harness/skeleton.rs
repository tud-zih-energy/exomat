//! harness skeleton subcommand

use std::path::Path;

use crate::experiment::ExperimentSource;
use crate::helper::errors::Result;

/// entrypoint for skeleton binary
pub fn main(exp_src_dir: &Path) -> Result<()> {
    let _ = ExperimentSource::create_skeleton(exp_src_dir.to_path_buf())?;

    println!(
        r#"
Next steps:

# add variables
exomat -C {dir} env --add COUNT 1 2 3

# adjust script
vim {dir}/template/run.sh

# execute experiment
exomat run {dir}
"#,
        dir = exp_src_dir.display()
    );

    Ok(())
}
