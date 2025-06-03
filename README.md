# exomat
The `exomat` (experiment-o-matic) is a simple tool for organized creation and running of experiments.

## Overview
`exomat` is a suite of tools for the entire experiment lifecycle:

- `skeleton`: Create a new experiment
- `env`: Modify environment variables/parameters available in experiments
- `run`: Execute an experiment
- `make-table`: Collect all results into a csv file
- `completion`: Generate shell completions

also see `--help`

## Usage Example
### Setup Experiment
```bash
# load completions
$ source <(exomat completion)
[2025-02-31 13:33:37.420] [info] Experiment harness created under loadavg

# run.sh is the entry point for your script
# the entire template directory will be replicaed for each experiment configuration
$ cat <<EOF >loadavg/template/run.sh
#!/usr/bin/env bash
set -euo pipefail
mpirun -n $NCPUS -- ~/my_benchmark
EOF
```

### Configure Parameters
```bash
# from inside loadavg dir
$ exomat env --add NCPUS 1 2 3 4

# call withouth arguments to show all configurations
$ exomat env
[2025-02-31 13:33:37.420] [info] 4 env files found
┌───────┬───────┐
│ file  │ NCPUS │
├───────┼───────┤
│ 0.env │ 1     │
│ 1.env │ 2     │
│ 2.env │ 3     │
│ 3.env │ 4     │
└───────┴───────┘
```

### Run Experiment
The directory `template/` will be cloned for each env file (and repetition).
The environment variables will be loaded, and then `run.sh` executed from this new directory.

```bash
# test a random configuration
$ exomat run loadavg --trial
[...]
[loadavg] returned:
Successful

# actually run experiment
$ exomat run loadavg --repetitions 3
[...]
[2025-02-31 13:33:37.420] [info] Created new experiment series dir at /tmp/loadavg-2025-02-31-13-33-37
[2025-02-31 13:33:37.420] [info] Starting experiment runs for /tmp/loadavg
[2025-02-31 13:33:37.420] [info] run_2_rep0 finished successfully with exit status: 0
[...]
[00:00:00] [####################] 12/12 (0s)

# there is now one dir per configuarion in `runs`
$ ls loadavg-2025-02-31-13-33-37/runs
run_0_rep0 run_0_rep1 run_0_rep2 run_1_rep0 [...]
```

### Collect Results
If the `run.sh` creates a file `out_myvar`, its content can be extracted with `exomat make-table`.
Variables configures via `exomat env` will automatically be included.

```bash
$ cd loadavg-2025-02-31-13-33-37
$ exomat make-table
[2025-02-31 13:33:37.420] [info] Collected output for 2 keys
[2025-02-31 13:33:37.420] [info] Found keys: ["myval", "NCPUS"]

$ cat loadavg-2025-02-31-13-33-37.csv 
myval,NCPUS
value,2
value,1
value,4
value,3
```

## Logging
The amount of log content on your console can be configured using the `-v` or `-q` flag.
Specify `-v` multiple times to increase verbosity.

> The progress bar printed by `exomat run` is not affected by this option. It will always be printed.

### Log Files
The `exomat run` command produces three different log files found under `[series]/runs/`.
- `stdout.log`: Output written to stdout by all run repetitions
- `stderr.log`: Output written to stderr by all run repetitions
- `exomat.log`: Log output from the exomat itself with timestamps (all levels)
    - Environment variables used for a run
    - Start of a run
    - End of a run
    - Exit code of a run
    - Did a run produce stderr output?

> This log content is also not affected by `-v` or `-q`.

## License
`exomat` is available under GPLv3+ (GPL-3.0-or-later).
