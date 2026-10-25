#!/usr/bin/env bash
#
# Runs shellcheck over every shell file the repository owns: the CI helpers
# and the developer scripts (make-icons.sh, bundle.sh).
#
# Skips silently when shellcheck is absent, so the gate still works on a
# machine or runner without it.

set -euo pipefail

if ! command -v shellcheck >/dev/null 2>&1; then
    echo "shellcheck not installed; skipping"
    exit 0
fi

shellcheck .github/scripts/*.sh scripts/*.sh

# Newer shellcheck drops checks rather than adding them (0.11 dropped SC2015),
# so a clean run locally can still be red on a runner pinned to an older build.
# Report the version so a CI failure can be reproduced with the same one.
shellcheck --version | grep version:
