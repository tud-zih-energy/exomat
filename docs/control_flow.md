# Control Flow
## `exomat run`
1. fetch environment
2. create experiment series directory
3. create a list of all experiment executions
4. shuffle this list
5. for each execution:
    1. Load serialized environment variables
    2. Load exomat's internal environment variables (only serialize specific ones)
    3. create experiment run directory
    4. execute run.sh
6. (only in trial runs) gather output

## `exomat env`
(specifically `--add` `--append` and `--remove`)
1. read existing environment variables from env files
2. edit environment variables
    1. add new variabels
    2. append values to existing variables
    3. remove variables and/or values
3. remove existing env files
4. serialize updated variables as new env files (keep the names)

## `exomat make-table`
1. locate all experiment run directories
2. gather all environment variables with values
3. gather content in `out_` files
4. format and print everything in a table

## `exomat skeleton`
1. create experiment source directory with all subdirectories
2. create empty `run.sh`
3. copy content from `run.sh.template` to `run.sh`
4. create empty `0.env`
