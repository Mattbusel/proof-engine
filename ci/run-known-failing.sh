#!/usr/bin/env sh
# Runs only the library tests listed in ci/known-failing-tests.txt.
set -eu
cd "$(dirname "$0")/.."
names=$(grep -v '^#' ci/known-failing-tests.txt | grep -v '^$')
# shellcheck disable=SC2086
exec cargo test --lib -- --exact $names
