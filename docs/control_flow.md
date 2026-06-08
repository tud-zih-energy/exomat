# Control Flow
## `exomat run`
1. create experiment series directory: `[experiment]-YYYY-MM-DD-HH-MM-SS`
2. fetch environments from `[experiment]/envs`
3. create a list of all experiment repetitions
    1. create one repetition per environemnt * repetition count
    2. shuffle this list
    3. sort by repetition number (all n-repetitions run before all n+1-repetitions)
4. for each repetition:
    1. Load exomat's internal environment variables
    2. create experiment run directory: `[experiment]-YYYY-MM-DD-HH-MM-SS/runs/run_[env]_rep[repetition]`
        1. copy `run.sh` and `[env].env` from `[experiment]-YYYY-MM-DD-HH-MM-SS/.src/`
        2. append exomat's internal environment variables to `[env].env`
    3. execute `run.sh`

### Behaviour in the error case
If any of the operations fails, especially if `run.sh` in step 4.3 returns a non-zero exit code, the execution fails fast, not executing the remaining repetitions or environments.

## `exomat run --trial`
1. create trial directory: `[temp_dir]/exomat_trial-YYYY-MM-DD-HH-MM-SS`
2. execute `exomat run` steps in trial directory for one repetition of one environment
3. gather and print output and log content


## `exomat env` with `--add` `--append` or `--remove`
1. read existing environment variables from `[experiment]/envs`
2. assert that no reserved environemnt variables will be changed
3. edit environment variables
    1. **add** new variabels
    2. **append** values to existing variables
    3. **remove** variables and/or values
3. remove existing env files
4. serialize updated variables as env files in `[experiment]/envs`

## `exomat make-table`
1. locate all experiment run directories in `[experiment]-YYYY-MM-DD-HH-MM-SS`
2. gather all environment variables with values from all repetitions
3. gather content in `out_` files
4. format and print everything in a table

## `exomat skeleton`
1. create experiment source directory with all subdirectories
2. create empty `run.sh`
3. copy content from `run.sh.template` to `run.sh`
4. create empty `0.env`

## Further information
For a more detailed explanation of all available subcommands and their options, see `exomat --help`
