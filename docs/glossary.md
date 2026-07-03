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
       |    |-> out_[var]
       |    \-> [...]
       |   # experiment run folder
       |-> run_[env_name]_rep[repetition2]
       |    |-> .exomat_run
       |    |-> run.sh
       |    |-> environment.env
       |    | # experiment output / out_ file
       |    |-> out_[var]
       |    \-> [...]
       |   # experiment run folder
       |-> run_[...]
       |-> stdout.log
       |-> stderr.log
       \-> exomat.log
```
---

An experiment run can generate multiple `Observation`s (e.g. multiple power readings during a single run).
In experiment runs with multiple `Observation`s, the `out_`-files will contain multiple values separated by newlines.

For example, given the following `out_`-files inside of an experiment run:
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

# run[...]/out_constant
hello world
```
This experiment run contains the `Observation`s:
```
[
  [0, 42, hello world],
  [1, 69, hello world],
  [2, 420, hello world],
  [3, 67, hello world]
]
```

**A few important things:**
1. If there are multiple `Observation`s in a run, then every `out_`-file has to contain either 1 or as many values/lines as every other `out_`-file
2. The nth `Observation` in an experiment run is made of the nth line of every out_ file
3. If there are multiple `Observation`s and there are `out_`-files with only a single value, every `Observation` has that same, single, value


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
name                 | module     | description
---------------------|------------|---------------
Environment          | env        | Content of one `.env`-file
EnvironmentContainer | env        | List of `.env`-files
ExomatEnvironment    | env        | List of Exomat-internal environment variables
ExperimentSource     | experiment | Internal representation of an Experiment Source
ExperimentSeries     | experiment | Internal representation of an Experiment Series
ExperimentRun        | experiment | Internal representation of an Experiment Run
OutFile              | experiment | Internal representation of an `out_`-file
OutList              | experiment | List of `out_`-files

> Some structs have iterator implementations. They use separate structs, called `[struct]Iter`. They are not listed here.

### Type Definitions
name                    | module     | alias                           | description
------------------------|------------|---------------------------------|--------------
Result                  | error      | `Result<T, Error>`              | Exomat return type
Observation             | experiment | `HashMap<String, String>`       | One row of values across all `out_`-files in an Experiment Run
EnvList                 | env        | `HashMap<String, Vec<String>>`  | Lists all possible `values` for each `NAME` (see `env_list`)
EnvironmentLocationList | env        | `HashMap<PathBuf, Environment>` | Maps File Paths to Environments
