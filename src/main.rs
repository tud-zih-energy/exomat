use clap::Parser;
use spdlog::prelude::{debug, error};
use std::process::ExitCode;

pub mod bin {
    pub mod cli_structure;
    pub mod completion;
    pub mod run;
}

use bin::cli_structure::{Cli, Commands};
use exomat::helper::errors::{Error, Result};

fn main() -> ExitCode {
    let args = Cli::parse();
    let log_handler = exomat::activate_logging(args.verbose.log_level_filter());

    match run_main(args, log_handler) {
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}

fn run_main(args: Cli, log_handler: indicatif::MultiProgress) -> Result<()> {
    if let Some(pwd) = args.cd {
        debug!("changing pwd to {}", pwd.display());
        std::env::set_current_dir(pwd)?;
    }

    match args.subcommand {
        Commands::Run {
            experiment,
            trial,
            output,
            repetitions,
        } => bin::run::main(experiment, trial, output, repetitions, log_handler),
        Commands::Skeleton { experiment } => exomat::harness::skeleton::main(&experiment),
        Commands::Env {
            add,
            append,
            remove,
        } => exomat::harness::env::main(add, append, remove),
        Commands::MakeTable {} => exomat::harness::table::main(),
        Commands::Completion { shell } => bin::completion::main(shell),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn log_handler() -> indicatif::MultiProgress {
        indicatif::MultiProgress::new()
    }

    #[test]
    fn change_working_directory() {
        let workspace = tempfile::tempdir().unwrap();
        std::env::set_current_dir(workspace.path()).unwrap();

        // initialize experiment dir
        let args = Cli::parse_from(&["argv0", "skeleton", "exp_dir"]);
        run_main(args, log_handler()).unwrap();

        // no cd: does not work
        let args = Cli::parse_from(&["argv0", "env", "--add", "VAR", "1", "2", "3"]);
        assert!(run_main(args, log_handler()).is_err());

        // but works with cd into new dir
        let args = Cli::parse_from(&[
            "argv0", "-C", "exp_dir", "env", "--add", "VAR", "1", "2", "3",
        ]);
        run_main(args, log_handler()).unwrap();
    }
}
