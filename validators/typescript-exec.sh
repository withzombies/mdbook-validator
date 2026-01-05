#!/bin/sh
#
# typescript-exec.sh - Container execution script for TypeScript validation.
#
# Reads TypeScript code from stdin, compiles with tsc --strict, runs with node,
# and outputs JSON: {"exit_code": N, "stdout": "...", "stderr": "..."}
#
# This script runs INSIDE the container. The host validator (validate-typescript.sh)
# parses the JSON output and checks assertions.
#
# Prerequisites in container:
# - node (runtime)
# - npx (to install typescript on demand)
#
# Note: SETUP runs before this script via preprocessor. If SETUP includes
# "npm init -y && npm install <package>", you must run that in /app for
# the dependencies to be found by this script.
#
# Example SETUP for npm dependencies:
#   cd /app && npm init -y && npm install axios @types/node
#

# Work in /app directory (where npm dependencies are installed)
# Create it if SETUP didn't already
mkdir -p /app
cd /app || exit 1

# Read TypeScript code from stdin
cat > script.ts

# Type check with tsc --strict
# tsc outputs errors to stdout (not stderr), so capture stdout
# Suppress npm notices by redirecting stderr to /dev/null
# Note: Don't use set -e because we need to capture tsc failures
npx -y -p typescript tsc --strict --lib es2020,dom script.ts > tsc_out.txt 2>/dev/null
TSC_EXIT=$?

if [ $TSC_EXIT -ne 0 ]; then
    # Type error - report tsc output as stderr in JSON
    # Escape for JSON: backslashes first, then quotes, remove newlines
    STDERR=$(cat tsc_out.txt | tr -d '\n' | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')
    printf '{"exit_code": %d, "stdout": "", "stderr": "%s"}' "$TSC_EXIT" "$STDERR"
else
    # Compilation succeeded - run the compiled JavaScript
    node script.js > stdout.txt 2> stderr.txt
    EXIT_CODE=$?

    # Escape outputs for JSON
    STDOUT=$(cat stdout.txt | tr -d '\n' | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')
    STDERR=$(cat stderr.txt | tr -d '\n' | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')

    printf '{"exit_code": %d, "stdout": "%s", "stderr": "%s"}' "$EXIT_CODE" "$STDOUT" "$STDERR"
fi
