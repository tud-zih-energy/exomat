# Harness

The ex-o-mat harness is a tool which should aid a researcher with running multiple repetitions of different experiments.

## Experiment

An experiment is a folder. The name of the folder is the name of the experiment.
The ex-o-mat makes no assumption about the relationship between different experiments. To it, every experiment is 100% different.

The structure of an experiment folder is as follows:

```
[experiment]/
  |-> template/
  |    |-> run.sh
  |    \-> [...]
  |-> envs/
  |    |-> 0.env
  |    |-> foobar.env
  |    \-> [...]
  \-> README
```

`template` contains the files needed to run the experiment. It atleast needs to contain a `run.sh` file which is the script started on every experiment. Beyond that, it can contain as many files as you like.

As the files in `template` are copied many times, it should only contain files for defining the run environment of the experiment. Executables, input data and the like should be stored outside of the experiment folder. However, everyone is free to shoot themselves in the foot with GiBs of template folders, so this rule is not actively enforced.

> Corollary: Static files should probably live outside the experiment directory.

`envs` contains the environment files (named `[something].env`) for the different runs of the experiment. Environment files are used to define different configurations of an experiment, such as  "Turbofrequencies enabled" and "Turbofrequencies disabled", for example.

All environment files are structured as a list of assignments:

```
FOO=BAR
BLA=BLUB
```

In the following, tools for generating env files will also be presented.

Every experiment has to have an `envs` folder, even if it contains just one empty .env file.

Lastly, the experiment has to contain a README file.

## Running an experiment

`exomat harness run [experiment]` runs an experiment.

### Options

- `exomat harness run -o [output_folder]`
  Sets a specific output directory instead of `[experiment]-YYYY-MM-DD-HH-MM-SS`
- `exomat harness run -r [repetitions]`
  Sets to run the experiment for `[repetitions]` repetitions.

### File output

Running an experiment produces the following file output

```
[experiment]-YYYY-MM-DD-HH-MM-SS
  |-> .src/
  |    |-> template/
  |    |    \-> run.sh
  |    \-> [...]
  \-> runs/
       |-> run_[env_name]_rep[repetition1]
       |    |-> run.sh
       |    |-> environment.env
       |    |-> my_special_output_file.txt
       |    \-> [...]
       |-> run_[env_name]_rep[repetition2]
       |    |-> run.sh
       |    |-> environment.env
       |    |-> my_special_output_file.txt
       |    \-> [...]
       |-> run_[...]
       |-> stdout.log
       |-> stderr.log
       \-> exomat.log
```

The `.src` folder contains a complete copy of the `[experiment]`.
It is designated as a backup in case the original source gets lost.
It is writeable, in order to not interfere with `rm -r` of the whole experiment series folder.
It is hidden, in order to dissuade users from interacting/changing it.
It contains a special marker file, which prevents it from being passed to the `run` subcommand directly.

The `runs` folder contains the separate runs of the experiment.

The repetition numbers are expanded, so that all numbers have the same length, making it easier to sort the folders.
If 1000 repetitions are given. the numbers run 000-...-023-...-999, not 0-...-23-...-999.

### Main Loop of the run command:

`exomat harness run [experiment] -r [repetitions]`
1. Create `[run folder]`: `run_[experiment]-YYYY-MM-DD-HH-MM-SS`
2. Create `[run folder]/src`, copy the content of `[experiment]` into it.
3. Create `[run folder]/runs`
4. Write console output of `exomat` also  to `[run_folder]/runs/exomat.log`
5. For every environment `[envname].env` in `[experiment]/envs`:
    1. for every repetition in `[repetitions]`:
        1. create a `[repetition folder]`: `[run folder]/runs/run_[envname]_rep[itnumber]`
        2. copy the content of `[experiment]/template` into the `[repetition_folder]`
        3. copy the `[experiment]/envs/[envname].env` file to `[repetition folder]/environment.env`
           > Note: This must be loaded inside `run.sh`!
        4. execute `run.sh` inside `[repetition folder]`
        5. append stderr and stdout of `run.sh` to `[run folder]/runs/stderr.log` and `[run folder]/runs/stdout.log`


### Behaviour  in the error case

If any of the operations fails, especially if `run.sh` in step 5.1.4 returns a non-zero exit code, the execution should fail fast, not executing the remaining repetitions or environments.

### Console Output

`exomat harness run` should not give any output from the `run.sh` script it executes. That is completely writting to `stderr.log` and `stdout.log`.

What `exomat` should give is clear indication of progress. maybe a progress bar, maybe log messages "Executed foo.env, repetition 3", be creative.

# Helper commands

## `exomat harness skeleton [experiment]`

Initializes a new [experiment] folder.

It is initialized as follows:
```
[experiment]/
  |-> template/
  |    \-> run.sh [EMPTY, EXECUTABLE]
  |-> envs/
  |    \-> 0.env [EMPTY]
  \-> README [EMPTY]
```

## `exomat harness env`

Handles (as of now: **generates**) env files _in the current directory_ according to the template.

- `--add VAR VAL1 VAL2 ...`:
  Adds a variable VAR.
  Can be specified multiple times.
  Aborts if `VAR` is already defined.

### Example

`exomat harness env --add FOO BAR BAZ --add X A B` generates 4 env files:

```
# 0.env
FOO=BAR
X=A
# 1.env
FOO=BAZ
X=A
# 2.env
FOO=BAR
X=B
# 3.env
FOO=BAZ
X=B
```
