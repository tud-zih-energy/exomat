#!/usr/bin/env bash
set -euo pipefail

#
# integration test of all harness commands
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
! "$EXOMAT_BIN" harness

# missing experiment
! "$EXOMAT_BIN" harness skeleton
! "$EXOMAT_BIN" harness env
! "$EXOMAT_BIN" harness run
# experiment is optional for harness make-table

# should work
"$EXOMAT_BIN" harness skeleton One
test -d $DIR/One
test -f $DIR/One/template/run.sh
test -f $DIR/One/envs/0.env

# env needs the --add option for now
! "$EXOMAT_BIN" harness env One
test -f $DIR/One/envs/0.env         # not deleted
test '!' -f $DIR/One/envs/1.env     # not created

# should work
"$EXOMAT_BIN" harness env One --add ONE foo,bar
test -f $DIR/One/envs/0.env
test -f $DIR/One/envs/1.env

# experiment does not exist
test '!' -d $DIR/non_existent
! "$EXOMAT_BIN" harness run non_existent

# should work
"$EXOMAT_BIN" harness run One
# from now on using -o, to access the output

# should work
"$EXOMAT_BIN" harness run One -o One_Out1
test -d $DIR/One_Out1
test -f $DIR/One_Out1/src/template/run.sh
test -f $DIR/One_Out1/src/envs/0.env
test -f $DIR/One_Out1/src/envs/1.env
test -f $DIR/One_Out1/runs/exomat.log
test -f $DIR/One_Out1/runs/stderr.log
test -f $DIR/One_Out1/runs/stdout.log
test -f $DIR/One_Out1/runs/run_0_rep0/run.sh
test -f $DIR/One_Out1/runs/run_0_rep0/environment.env
test -f $DIR/One_Out1/runs/run_1_rep0/run.sh
test -f $DIR/One_Out1/runs/run_1_rep0/environment.env

# only checking for correct number of repetitions
"$EXOMAT_BIN" harness run One -o One_Out2 -r 3
test -d $DIR/One_Out2/runs/run_0_rep0/
test -d $DIR/One_Out2/runs/run_1_rep0/
test -d $DIR/One_Out2/runs/run_0_rep1/
test -d $DIR/One_Out2/runs/run_1_rep1/
test -d $DIR/One_Out2/runs/run_0_rep2/
test -d $DIR/One_Out2/runs/run_1_rep2/
test '!' -d $DIR/One_Out2/runs/run_0_rep3/
test '!' -d $DIR/One_Out2/runs/run_1_rep3/

# output already exists
! "$EXOMAT_BIN" harness run One -o One_Out2

# should work
"$EXOMAT_BIN" harness make-table One_Out1
test -f $DIR/One_Out1/One_Out1.csv
test "$(cat $DIR/One_Out1/One_Out1.csv)" = ""

# create mock output
echo 0 > $DIR/One_Out1/runs/run_0_rep0/out_foo
echo a > $DIR/One_Out1/runs/run_0_rep0/out_bar
echo 1 > $DIR/One_Out1/runs/run_1_rep0/out_foo
echo b > $DIR/One_Out1/runs/run_1_rep0/out_bar

# should work
"$EXOMAT_BIN" harness make-table One_Out1
test -f $DIR/One_Out1/One_Out1.csv
test "$(cat $DIR/One_Out1/One_Out1.csv)" = "foo,bar
0,a
1,b"
