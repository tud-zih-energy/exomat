#!/usr/bin/env bash
set -euo pipefail

# this script creates a directory with the correct structure to turn it into a
# .deb/.rpm package using fpm

# check for deb/fpm
PACKAGE_TYPE=$1
test "$PACKAGE_TYPE" = "deb" -o "$PACKAGE_TYPE" = "rpm" || (echo "Unsupported package type: $PACKAGE_TYPE" >&2 && exit 127)

# read which binary to use
EXOMAT_BIN_DIR=$2
test -d $EXOMAT_BIN_DIR || (echo "exomat binary dir not a directory: $EXOMAT_BIN_DIR" >&2 && exit 127)
test -f "$EXOMAT_BIN_DIR/exomat" -a -x "$EXOMAT_BIN_DIR/exomat" || (echo "exomat binary dir found at: $EXOMAT_BIN_DIR/exomat" >&2 && exit 127)

# set version
PACKAGE_VERSION="0.1.0"

# get target architecture, build name
if [ "$(file $EXOMAT_BIN_DIR/exomat | grep x86-64)" ]
then
  PACKAGE_ARCH="amd64"
elif [ "$(file $EXOMAT_BIN_DIR/exomat | grep aarch64)" ]
then
  PACKAGE_ARCH="aarch64"
fi

PACKAGE_NAME="exomat-$PACKAGE_VERSION-$PACKAGE_ARCH"

# create PKG_DIR and step into
PKG_DIR="./into_deb/"
mkdir $PKG_DIR
PKG_DIR="$(readlink -f $PKG_DIR)"
cd $PKG_DIR

# delete dir on close and print exit code
function cleanup()
{
    echo "exit code:    " $?

    # try to remove $PKG_DIR from the current location, don't show error output
    rm -r $PKG_DIR &>/dev/null
}
trap cleanup EXIT SIGINT SIGTERM

#create all needed directories
mkdir -p ./usr/bin
mkdir -p ./usr/share/doc/exomat

# copy files
cp "$EXOMAT_BIN_DIR/exomat" ./usr/bin/exomat    # binary
cp ../README.md ./usr/share/doc/exomat/README   # readme

# set permissions explicitly
chmod 755 ./usr/bin/exomat
chmod 644 ./usr/share/doc/exomat/README

# try to add autocompletion, only automatic for bash
if [ -f ../target/exomat.bash ];
then
  mkdir -p ./usr/share/bash-completion/completions
  cp ../target/exomat.bash ./usr/share/bash-completion/completions/exomat.bash
  chmod 644 ./usr/share/bash-completion/completions/exomat.bash
else
 echo "You're using an unsupported shell, place the autocompletion file (found under target/exomat.*) yourself."
fi

cd ..

# call on fpm
fpm \
  -s dir -t $PACKAGE_TYPE \
  -p $PACKAGE_NAME.$PACKAGE_TYPE \
  -n exomat \
  -v $PACKAGE_VERSION \
  -a $PACKAGE_ARCH \
  --rpm-tag '%define _build_id_links none' \
  --rpm-tag '%undefine _missing_build_ids_terminate_build' \
  --description "Tools for running experiments" \
  # --url "" \
  --maintainer "Tessa Todorowski <tessa.todorowski@tu-dresden.de>" \
  --force \
  --chdir $PKG_DIR .

