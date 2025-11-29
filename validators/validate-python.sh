#!/bin/bash
#
# validate-python.sh - Host-based Python syntax validator.
#
# This script validates python py_compile output from the container.
# py_compile runs in the container, its stderr is passed via
# VALIDATOR_CONTAINER_STDERR. Runs on the HOST (not in container).
#
# Input: py_compile output via stdin (container stdout)
# Environment:
# - VALIDATOR_CONTAINER_STDERR: Container stderr, where py_compile writes errors
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
#
# Exits 0 on success, 1 on failure with details to stderr.
#

set -e

# Read stdin (py_compile output from container)
OUTPUT=$(cat)

# Check VALIDATOR_CONTAINER_STDERR for Python errors
if [ -n "${VALIDATOR_CONTAINER_STDERR:-}" ]; then
    # Check for Python compile error patterns:
    # - SyntaxError: invalid syntax
    # - IndentationError: unexpected indent (subclass of SyntaxError)
    # - TabError: inconsistent use of tabs and spaces (subclass of SyntaxError)
    if echo "$VALIDATOR_CONTAINER_STDERR" | grep -qE "(SyntaxError|IndentationError|TabError)"; then
        echo "Python validation failed:" >&2
        echo "$VALIDATOR_CONTAINER_STDERR" >&2
        exit 1
    fi
fi

# If no assertions, we're done (py_compile passed in container)
if [ -z "${VALIDATOR_ASSERTIONS:-}" ]; then
    exit 0
fi

# Evaluate assertions if provided
while IFS= read -r assertion || [ -n "$assertion" ]; do
    # Skip empty lines and trim whitespace
    assertion=$(echo "$assertion" | xargs 2>/dev/null || echo "$assertion")
    [ -z "$assertion" ] && continue

    case "$assertion" in
        contains\ *)
            needle=${assertion#contains }
            # Remove surrounding quotes if present
            needle=${needle#\"}
            needle=${needle%\"}
            # Check if the string appears in the output
            if ! echo "$OUTPUT" | grep -qF "$needle"; then
                # Also check stderr
                if ! echo "${VALIDATOR_CONTAINER_STDERR:-}" | grep -qF "$needle"; then
                    echo "Assertion failed: contains \"$needle\": not found in output" >&2
                    exit 1
                fi
            fi
            ;;
        *)
            echo "Assertion failed: Unknown assertion syntax: $assertion" >&2
            echo "Supported assertions for python: contains \"string\"" >&2
            exit 1
            ;;
    esac
done <<< "$VALIDATOR_ASSERTIONS"

exit 0
