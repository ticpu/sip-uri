#!/bin/bash
# Install git hooks: run them straight from this directory via core.hooksPath.

set -e

git rev-parse --git-dir >/dev/null || {
    echo "Error: Not in a git repository"
    exit 1
}

git config core.hooksPath hooks
echo "core.hooksPath set to hooks/ (pre-commit, pre-push active)"
