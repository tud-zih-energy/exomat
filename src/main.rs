use clap::Parser;
use spdlog::prelude::error;
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

    let res = match args.subcommand {
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
        Commands::MakeTable {} => exomat::make_table(),
        Commands::Completion { shell } => bin::completion::main(shell),
    };

    match res {
        Err(err) => {
            error!("{err}");
            ExitCode::FAILURE
        }
        Ok(()) => ExitCode::SUCCESS,
    }
}
