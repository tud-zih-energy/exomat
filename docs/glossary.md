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
       |    | # experiment output / out_ file
       |    |-> out_put.txt
       |    \-> [...]
       |   # experiment run folder
       |-> run_[env_name]_rep[repetition2]
       |    |-> .exomat_run
       |    |-> run.sh
       |    |-> environment.env
       |    | # experiment output / out_ file
       |    |-> out_put.txt
       |    \-> [...]
       |   # experiment run folder
       |-> run_[...]
       |-> stdout.log
       |-> stderr.log
       \-> exomat.log
```
---
Inside of an out_ file, there can be multiple `Observations`.
For example, given the following out_ files inside of an experiment run:
```bash
# run[...]/out_duration
0
1
2
3

# run[...]/out_value
42
69
420
67
```
This experiment run contains the Observations:
`[0, 42], [1,69], [2,420], [3,67]`

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
