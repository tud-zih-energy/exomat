# Glossary
## Experiment files and directories
```bash
# experiment source folder
[experiment]/
  |-> .exomat_source
  |-> template/
  |    |-> run.sh
  |    \-> [...]
  |-> envs/
  |    |-> 0.env
  |    |-> foobar.env
  |    \-> [...]
  \-> ...

# experiment series folder
[experiment]-YYYY-MM-DD-HH-MM-SS
  |-> .exomat_series
  |   # .src/ directory
  |-> .src/
  |    |-> .exomat_source_cp
  |    |-> template/
  |    |    \-> run.sh
  |    \-> [...]
  |   # runs/ directory
  \-> runs/
       |   # experiment run folder
       |-> run_[env_name]_rep[repetition1]
       |    |-> .exomat_run
       |    |-> run.sh
       |    |-> environment.env
       |    |-> my_special_output_file.txt
       |    \-> [...]
       |   # experiment run folder
       |-> run_[env_name]_rep[repetition2]
       |    |-> .exomat_run
       |    |-> run.sh
       |    |-> environment.env
       |    |-> my_special_output_file.txt
       |    \-> [...]
       |   # experiment run folder
       |-> run_[...]
       |-> stdout.log
       |-> stderr.log
       \-> exomat.log
```

### Experiment Source
An experiment source is a folder. The name of the folder is the name of the experiment.
The ex-o-mat makes no assumption about the relationship between different experiment sources. To it, every experiment source is 100% different.

The structure of an experiment source folder can be seen above.

`template` contains the files needed to run the experiment. It atleast needs to contain a `run.sh` file which is the script started on every experiment. Beyond that, it can contain as many files as you like.

As the files in `template` are copied many times, it should only contain files for defining the run environment of the experiment. Executables, input data and the like should be stored outside of the experiment folder. However, everyone is free to shoot themselves in the foot with GiBs of template folders, so this rule is not actively enforced.

> Corollary: Static files should probably live outside the experiment directory.

`envs` contains the environment files (named `[something].env`) for the different runs of the experiment. Environment files are used to define different configurations of an experiment, such as  "Turbofrequencies enabled" and "Turbofrequencies disabled", for example.
All environment files are structured as a list of assignments:

```bash
FOO=BAR
BLA=BLUB
```

Every experiment has to have an `envs` folder, even if it contains just one empty .env file.

### Experiment Series
Running an experiment produces an experiment series folder. It's structure can be seen above.

The `.src` folder contains a complete copy of the `[experiment]`.
It is designated as a backup in case the original source gets lost.
It is writeable, in order to not interfere with `rm -r` of the whole experiment series folder.
It is hidden, in order to dissuade users from interacting/changing it.
It contains a special marker file, which prevents it from being passed to the `run` subcommand directly.

The `runs` folder contains the separate runs of the experiment.

The repetition numbers in the name of an experiment series are expanded, so that all numbers have the same length, making it easier to sort the folders.
If 1000 repetitions are given. the numbers run 000-...-023-...-999, not 0-...-23-...-999.

`exomat run` does not give any output from the `run.sh` script it executes. That is completely written to `stderr.log` and `stdout.log`.
Additionally, any `exomat` output during the experiment is written to `exomat.log`.

## Environments
If a function contains one of the following words, this is what you can expect:

name                    | meaning                                         | possible use
------------------------|-------------------------------------------------|---------------------
`env`                   | A single `NAME=value` pair                      |
`env_var`               | The `NAME` of an env                            |
`env_val`               | The `value` of an env                           |
`env_list`              | A list of all possible `values` for each `NAME` |
`environment`           | A list of `NAME=value` pairs                    | content of an .env file
`environment_list`      | A list of `(NAME, value)` pairs                 | dotenvy vars
`environment_container` | A list of environments                          | directory of .env files
`exomat_environment`    | An environment with variables set by exomat     |

## Types and Structs
There are different types and structs defined by the exomat.
As to not confuse them, here is a list of all defined types/structs:

> These names should never be used for anything other than what they describe here

### Structs
name                 | module | description
---------------------|--------|---------------
Environment          | env    | Content of one `.env`-file
EnvironmentContainer | env    | List of `.env`-files
ExomatEnvironment    | env    | List of Exomat-internal environment variables

### Type Definitions
name                    | module | alias                           | description
------------------------|--------|---------------------------------|--------------
EnvList                 | env    | `HashMap<String, Vec<String>>`  | Lists all possible `values` for each `NAME` (see `env_list`)
EnvironmentLocationList | env    | `HashMap<PathBuf, Environment>` | Maps File Paths to Environments
