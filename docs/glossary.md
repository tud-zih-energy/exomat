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
