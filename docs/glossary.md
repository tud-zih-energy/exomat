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

## Environments
If a function contains one of the following words, this is what you can expect:

name                    | meaning                                         | possible use
------------------------|-------------------------------------------------|---------------------
`env`                   | A single `NAME=value` pair                      |
`env_var`               | The `NAME` of an env                            |
`env_val`               | The `value` of an env                           |
`env_list`              | A list of all possible `values` for each `NAME` |
`environment`           | A list of `NAME=value` pairs                    | content of an .env file
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
