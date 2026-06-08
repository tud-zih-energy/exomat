## Running an experiment

`exomat harness run [experiment]` runs an experiment.

### Options

- `exomat harness run -o [output_folder]`
  Sets a specific output directory instead of `[experiment]-YYYY-MM-DD-HH-MM-SS`
- `exomat harness run -r [repetitions]`
  Sets to run the experiment for `[repetitions]` repetitions.


### Main Loop of the run command:

`exomat harness run [experiment] -r [repetitions]`
1. Create `[run folder]`: `[experiment]-YYYY-MM-DD-HH-MM-SS`
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

# Helper commands

## `exomat harness skeleton [experiment]`

Initializes a new [experiment] folder.

It is initialized as follows:

```bash
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

```bash
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
