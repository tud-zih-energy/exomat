use crate::{Error, Result};
use clap::CommandFactory;
use clap_complete::{generate, Shell};

use super::cli_structure::Cli;

pub fn main(shell: Option<Shell>) -> Result<()> {
    let shell = match shell {
        Some(x) => x,
        None => clap_complete::Shell::from_env().ok_or(Error::CompletionError {
            err: "unknown shell (check $SHELL?)".to_string(),
        })?,
    };

    let mut cmd = Cli::command();
    // copy to separate var to please borrow checker
    let cmd_name = cmd.get_name().to_string();
    generate(shell, &mut cmd, cmd_name, &mut std::io::stdout());
    Ok(())
}
