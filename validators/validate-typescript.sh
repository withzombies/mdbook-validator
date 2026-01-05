#!/bin/bash
#
# validate-typescript.sh - Host-based TypeScript validator.
#
# This script validates TypeScript execution results from the container.
# Container runs tsc + node and outputs JSON: {"exit_code": N, "stdout": "...", "stderr": "..."}
# Validator parses JSON and checks assertions.
#
# Input: JSON via stdin (from container execution)
# Environment:
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
#   - exit_code = N: Script must exit with code N
#   - contains "string": Stdout must contain string
#   - stdout_contains "string": Stdout must contain string (alias for contains)
#   - rows = N: If stdout is JSON array, must have N elements
#
# Exits 0 on success, 1 on failure with details to stderr.
#

set -e

# Validate that a string is an integer (positive or negative)
is_integer() {
    [[ "$1" =~ ^-?[0-9]+$ ]]
}

# Check jq is available
command -v jq >/dev/null 2>&1 || {
    echo "ERROR: jq is required but not installed" >&2
    exit 1
}

# Read JSON from stdin
JSON_INPUT=$(cat)

# Validate JSON is parseable
if ! echo "$JSON_INPUT" | jq empty 2>/dev/null; then
    echo "ERROR: Invalid JSON from container" >&2
    echo "Received: $JSON_INPUT" >&2
    exit 1
fi

# Parse fields from JSON with fallbacks for missing fields
EXIT_CODE=$(echo "$JSON_INPUT" | jq -r '.exit_code // 0')
STDOUT=$(echo "$JSON_INPUT" | jq -r '.stdout // ""')
STDERR=$(echo "$JSON_INPUT" | jq -r '.stderr // ""')

# Track if we have an exit_code assertion
HAS_EXIT_CODE_ASSERTION=false

# Evaluate assertions if provided
if [ -n "${VALIDATOR_ASSERTIONS:-}" ]; then
    while IFS= read -r assertion || [ -n "$assertion" ]; do
        # Skip empty lines and trim leading/trailing whitespace (preserve quotes)
        assertion="${assertion#"${assertion%%[![:space:]]*}"}"  # trim leading
        assertion="${assertion%"${assertion##*[![:space:]]}"}"  # trim trailing
        [ -z "$assertion" ] && continue

        case "$assertion" in
            exit_code\ =\ *)
                HAS_EXIT_CODE_ASSERTION=true
                expected=${assertion#exit_code = }
                if ! is_integer "$expected"; then
                    echo "Assertion failed: exit_code = $expected: invalid integer" >&2
                    exit 1
                fi
                if [ "$EXIT_CODE" -ne "$expected" ]; then
                    echo "Assertion failed: exit_code = $expected: got $EXIT_CODE" >&2
                    if [ -n "$STDERR" ]; then
                        echo "stderr: $STDERR" >&2
                    fi
                    exit 1
                fi
                ;;
            contains\ *|stdout_contains\ *)
                # Handle both contains and stdout_contains (alias)
                if [[ "$assertion" == stdout_contains\ * ]]; then
                    needle=${assertion#stdout_contains }
                else
                    needle=${assertion#contains }
                fi
                # Remove surrounding quotes if present
                needle=${needle#\"}
                needle=${needle%\"}
                if ! echo "$STDOUT" | grep -qF "$needle"; then
                    echo "Assertion failed: contains \"$needle\": not found" >&2
                    echo "stdout: $STDOUT" >&2
                    exit 1
                fi
                ;;
            rows\ =\ *)
                expected=${assertion#rows = }
                if ! is_integer "$expected"; then
                    echo "Assertion failed: rows = $expected: invalid integer" >&2
                    exit 1
                fi
                # Parse stdout as JSON array and count elements
                if ! actual=$(echo "$STDOUT" | jq 'length' 2>/dev/null); then
                    echo "Assertion failed: rows = $expected: stdout is not valid JSON array" >&2
                    echo "stdout: $STDOUT" >&2
                    exit 1
                fi
                if [ "$actual" -ne "$expected" ]; then
                    echo "Assertion failed: rows = $expected: got $actual" >&2
                    exit 1
                fi
                ;;
            *)
                echo "Assertion failed: Unknown assertion syntax: $assertion" >&2
                echo "Supported: exit_code = N, contains \"str\", stdout_contains \"str\", rows = N" >&2
                exit 1
                ;;
        esac
    done <<< "$VALIDATOR_ASSERTIONS"
fi

# Default behavior: require exit code 0 if no exit_code assertion
if [ "$HAS_EXIT_CODE_ASSERTION" = false ]; then
    if [ "$EXIT_CODE" -ne 0 ]; then
        echo "Script failed with exit code $EXIT_CODE (expected 0)" >&2
        if [ -n "$STDERR" ]; then
            echo "stderr: $STDERR" >&2
        fi
        exit 1
    fi
fi

exit 0
