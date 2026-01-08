use clap::{Parser, Subcommand};
use clap_complete::Shell;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use std::path::PathBuf;

/// Tools for running experiments
///
/// Copyright (C) 2025 Tessa Todorowski
///
/// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
///
/// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
///
/// You should have received a copy of the GNU General Public License along with this program.  If not, see <https://www.gnu.org/licenses/>.
#[derive(Parser, Debug)]
#[command(version, name = "exomat")]
pub struct Cli {
    /// Subcommand to execute
    #[clap(subcommand)]
    pub subcommand: Commands,

    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initializes a new (and empty) EXPERIMENT folder.
    ///
    /// Uses the following structure:
    ///
    /// EXPERIMENT/
    ///   |-> template/
    ///   |    \-> run.sh [EMPTY, EXECUTABLE]
    ///   \-> envs/
    ///        \-> 0.env [EMPTY]
    ///
    /// Custom templates can be defined by creating the desired structure in
    /// `$HOME/.config/exomat/template`. If this folder exists, its contents
    /// will be copied to `EXPERIMENT/template` instead of using default-generation.
    #[command(verbatim_doc_comment)]
    Skeleton {
        /// Path to the experiment.
        ///
        /// Will create and populate a directory with this name.
        /// Automatically creates parent directories.
        #[clap()]
        experiment: PathBuf,
    },

    /// Handles env files in the current directory according to the template.
    ///
    /// Prints all environments as table if no modifications are given.
    Env {
        /// Adds a variable (first arg) with values (remaining args) to every .env
        /// file in the directory.
        ///
        /// Specify as many values as you want. Every value will result in new .env
        /// files being created, to generate every possible combination of the
        /// given values.
        ///
        /// For example, if `0.env` contains `FOO=bar` and `1.env`
        /// contains `FOO=foo` before you execute `exomat harness env --add BAZ 42 69`,
        /// the following files will be present after execution:
        /// - 0.env with `FOO=bar`, `BAZ=42`
        /// - 1.env with `FOO=foo`, `BAZ=42`
        /// - 2.env with `FOO=bar`, `BAZ=69`
        /// - 3.env with `FOO=foo`, `BAZ=69`
        /// > The order of files created does not necessarily represent reality
        ///
        /// Aborts if the variable is already defined.
        #[arg(short = 'a', long, num_args = 2..)]
        add: Vec<Vec<String>>,

        /// Edits a variable (first arg) by changing it's values (remaining args) in every .env
        /// file in the directory.
        ///
        /// The variable has to be given in at least one .env file to be able to
        /// append any values.
        ///
        /// Specify as many values as you want. Values that were set prior to this call will
        /// not be deleted. If a given value is already set, it will be skipped.
        ///
        /// For example, if `0.env` contains `FOO=bar, BAR=42` and `1.env`
        /// contains `FOO=foo, BAR=42` before you execute `exomat harness env --append BAZ 69`,
        /// the following files will be present after execution:
        /// - 0.env with `FOO=bar`, `BAZ=42`
        /// - 1.env with `FOO=foo`, `BAZ=42`
        /// - 2.env with `FOO=bar`, `BAZ=69`
        /// - 3.env with `FOO=foo`, `BAZ=69`
        /// > The order of files created does not necessarily represent reality
        #[arg(short = 'A', long, num_args = 2..)]
        append: Vec<Vec<String>>,

        /// Edits a variable (first arg) by removing it's values (remaining args)
        /// or the variable itself in every .env file in the directory.
        ///
        /// The variable has to be given in at least one .env file to be able to
        /// remove any values. The values also have to exist prior to calling this.
        ///
        /// Specify as many values as you want. Duplicate values will be skipped.
        ///
        /// For example, if `0.env` contains `FOO=bar, BAR=42` and `1.env`
        /// contains `FOO=foo, BAR=42` before you execute `exomat harness env --remove BAZ`,
        /// the following files will be present after execution:
        /// - 0.env with `FOO=bar`
        /// - 1.env with `FOO=foo`
        /// > The order of these files does not necessarily represent reality
        #[arg(short = 'r', long, num_args = 1..)]
        remove: Vec<Vec<String>>,
    },

    /// Execute an experiment from an experiment directory
    Run {
        /// Path to the experiment to run. Try PWD if not given.
        ///
        /// This is the path to a folder whose content conforms to the standards
        /// defined in `docs/harness.md`.
        #[clap()]
        experiment: PathBuf,

        /// Start a trial run of the experiment.
        ///
        /// Executes one run of an experiment with one env combination. The resulting
        /// experiment series directory will be deleted after completing the run.
        ///
        /// The exomat will then report on:
        /// - exit code of `run.sh`
        /// - content of exomat/stdout/stderr.log
        ///
        /// Custom output directories and repetition counts will be ignored.
        #[arg(short = 't', long, default_value_t = false)]
        trial: bool,

        /// Output folder.
        ///
        /// Sets a specific output directory instead of `[experiment]-YYYY-MM-DD-HH-MM-SS`.
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,

        /// Number of runs per experiment.
        ///
        /// This defines the number of directories inside the `[output]/runs/`
        /// directory. Each repetition will create a folder in the format `run_[env_name]_rep[Number]`.
        /// This format cannot be customized.
        #[arg(short = 'r', long, default_value_t = 1)]
        repetitions: u64,
    },

    /// Parses values from multiple output files into one file.
    ///
    /// Uses pwd as a starting point.
    ///
    /// For correct parsing: location / name of your output files need to conform to
    /// this format: ./runs/run_*/out_*
    MakeTable {},

    /// Generate exomat autocompletions
    ///
    /// Autocompletion will be printed to stdout. Example usage for bash:
    /// `source <(exomat completion)`
    ///
    /// `exomat completion > /usr/share/bash-completion/completions/exomat.bash`
    /// (open new shell afterwards)
    Completion {
        /// Shell to generate for
        ///
        /// Tries to use current shell by default.
        shell: Option<Shell>,
    },
}
