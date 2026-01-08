#!/usr/bin/env bash
set -euo pipefail

#
# integration test of all  commands
#

# get hwmondump binary from console input
EXOMAT_BIN=$1

test -f "$EXOMAT_BIN" -a -x "$EXOMAT_BIN" || (echo "exomat binary dir found at: $EXOMAT_BIN" >&2 && exit 127)

# make temp dir
DIR=$(mktemp -d)

# delete said temp dir on close
function cleanup()
{
    echo "exit code:    " $?
    rm -rf $DIR
}
trap cleanup EXIT SIGINT SIGTERM

# go to temp dir for simpler tests
cd $DIR

# missing subcommand
! "$EXOMAT_BIN"

! "$EXOMAT_BIN" skeleton     # missing experiment name
! "$EXOMAT_BIN" env          # not in an exmerpiment source directory
! "$EXOMAT_BIN" run          # missing experiment name
! "$EXOMAT_BIN" make-table   # not in an experiment series directory

#
# skeleton
#

# should work
"$EXOMAT_BIN" skeleton One
test -d $DIR/One
test -f $DIR/One/.exomat_source
test -f $DIR/One/template/run.sh
test -f $DIR/One/envs/0.env

# already exists
! "$EXOMAT_BIN" skeleton One

#
# env
#

# env needs to be called in an experiment source
! "$EXOMAT_BIN" env One

cd $DIR/One
    # should work
    "$EXOMAT_BIN" env

    "$EXOMAT_BIN" env --add ONE foo bar
    test -f $DIR/One/envs/0.env
    test -f $DIR/One/envs/1.env
    test '!' -f $DIR/One/envs/2.env

    "$EXOMAT_BIN" env --append ONE baz
    test -f $DIR/One/envs/0.env
    test -f $DIR/One/envs/1.env
    test -f $DIR/One/envs/2.env

    "$EXOMAT_BIN" env --remove ONE foo
    test -f $DIR/One/envs/0.env
    test -f $DIR/One/envs/1.env
    test '!' -f $DIR/One/envs/2.env

    "$EXOMAT_BIN" env --remove ONE
    test -f $DIR/One/envs/0.env
    test "$(cat $DIR/One/envs/0.env)" = ""
    test '!' -f $DIR/One/envs/1.env
    test '!' -f $DIR/One/envs/2.env

    # setup for run
    "$EXOMAT_BIN" env --add ONE foo bar
cd $DIR

#
# run
#

# experiment does not exist
test '!' -d $DIR/non_existent
! "$EXOMAT_BIN" run non_existent

# cannot be called in source
cd $DIR/One
    ! "$EXOMAT_BIN" run .
cd $DIR

# should work
"$EXOMAT_BIN" run One
# from now on using -o, to access the output

# should work
"$EXOMAT_BIN" run One -o One_Out1
test -d $DIR/One_Out1
test -f $DIR/One_Out1/.exomat_series
test -f $DIR/One_Out1/.src/template/run.sh
test -f $DIR/One_Out1/.src/.exomat_source_copy
test -f $DIR/One_Out1/.src/envs/0.env
test -f $DIR/One_Out1/.src/envs/1.env
test -f $DIR/One_Out1/runs/exomat.log
test -f $DIR/One_Out1/runs/stderr.log
test -f $DIR/One_Out1/runs/stdout.log
test -f $DIR/One_Out1/runs/run_0_rep0/.exomat_run
test -f $DIR/One_Out1/runs/run_0_rep0/run.sh
test -f $DIR/One_Out1/runs/run_0_rep0/environment.env
test -f $DIR/One_Out1/runs/run_1_rep0/.exomat_run
test -f $DIR/One_Out1/runs/run_1_rep0/run.sh
test -f $DIR/One_Out1/runs/run_1_rep0/environment.env

# only checking for correct number of repetitions
"$EXOMAT_BIN" run One -o One_Out2 -r 3
test -d $DIR/One_Out2/runs/run_0_rep0/
test -d $DIR/One_Out2/runs/run_1_rep0/
test -d $DIR/One_Out2/runs/run_0_rep1/
test -d $DIR/One_Out2/runs/run_1_rep1/
test -d $DIR/One_Out2/runs/run_0_rep2/
test -d $DIR/One_Out2/runs/run_1_rep2/
test '!' -d $DIR/One_Out2/runs/run_0_rep3/
test '!' -d $DIR/One_Out2/runs/run_1_rep3/

# trying to run a non-source directory
! "$EXOMAT_BIN" run One_Out1

# output already exists
! "$EXOMAT_BIN" run One -o One_Out2

#
# make-table
#

# needs to be called in a series directory
! "$EXOMAT_BIN" make-table One_Out1

cd $DIR/One_Out1
    # should work
    "$EXOMAT_BIN" make-table
    test -f $DIR/One_Out1/One_Out1.csv
    test "$(cat $DIR/One_Out1/One_Out1.csv)" != "" # contains envs

    # create mock output
    echo sentinel_0 > $DIR/One_Out1/runs/run_0_rep0/out_foo
    echo sentinel_a > $DIR/One_Out1/runs/run_0_rep0/out_bar
    echo sentinel_1 > $DIR/One_Out1/runs/run_1_rep0/out_foo
    echo sentinel_b > $DIR/One_Out1/runs/run_1_rep0/out_bar

    # should work
    "$EXOMAT_BIN"  make-table
    test -f $DIR/One_Out1/One_Out1.csv

    # strings must be contained (otherwise grep fails)
    grep sentinel_0 $DIR/One_Out1/One_Out1.csv > /dev/null
    grep sentinel_1 $DIR/One_Out1/One_Out1.csv > /dev/null
    grep sentinel_a $DIR/One_Out1/One_Out1.csv > /dev/null
    grep sentinel_b $DIR/One_Out1/One_Out1.csv > /dev/null
cd $DIR
