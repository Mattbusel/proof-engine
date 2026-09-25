#!/usr/bin/env sh
# Runs the library unit tests, skipping the names in ci/known-failing-tests.txt.
set -eu
cd "$(dirname "$0")/.."
skips=""
for name in $(grep -v '^#' ci/known-failing-tests.txt | grep -v '^$'); do
  skips="$skips --skip $name"
done
# shellcheck disable=SC2086
exec cargo test --lib -- --exact $skips
