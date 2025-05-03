#!/bin/sh

target_dir="${CARGO_TARGET_DIR}"
[ -z "${target_dir}" ] && target_dir="./target"

if [ "${1}" = "--release" ] ; then
  opt="${1}"
  target_dir="${target_dir}/release"
  shift
else
  opt=""
  target_dir="${target_dir}/debug"
fi

crate_name="game_client"
lib_name="lib${crate_name}.so"
[ "`uname -s`" = "Darwin" ] && lib_name="lib${crate_name}.dylib"

python="python3"
which "${python}" >/dev/null 2>&1 || python="python"

cargo build ${opt}

if [  "$?" = "0" ] ; then
  if [ "`uname -s`" = "Darwin" ] ; then
    export DYLD_LIBRARY_PATH="${DYLD_LIBRARY_PATH}:${target_dir}"
  else
    export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:${target_dir}"
  fi
  ${python} NtvPy.py ${crate_name} "${@}"
fi

