#!/bin/bash
#
# validate-shellcheck.sh - Host-based shellcheck output validator.
#
# This script validates shellcheck output from the container. Shellcheck runs
# in the container against a temp file, and its output (stdout+stderr combined)
# is passed to this validator. It runs on the HOST (not in container).
#
# Input: Shellcheck output via stdin (combined stdout/stderr from container)
# Environment:
# - VALIDATOR_CONTAINER_STDERR: Container stderr, where shellcheck writes findings
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
#
# Exits 0 on success, 1 on failure with details to stderr.
#

set -e

# Read stdin (shellcheck output from container)
OUTPUT=$(cat)

# Shellcheck writes findings to stderr. If there's anything in container stderr,
# check if it contains shellcheck findings (SC codes or line references)
if [ -n "${VALIDATOR_CONTAINER_STDERR:-}" ]; then
    # Check for shellcheck error patterns:
    # - "In script.sh line N:" format
    # - SC codes like SC2086
    if echo "$VALIDATOR_CONTAINER_STDERR" | grep -qE "(^In .* line [0-9]+:|SC[0-9]{4})"; then
        echo "Shellcheck found issues:" >&2
        echo "$VALIDATOR_CONTAINER_STDERR" >&2
        exit 1
    fi
fi

# If no assertions, we're done (shellcheck passed in container)
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
            echo "Supported assertions for shellcheck: contains \"string\"" >&2
            exit 1
            ;;
    esac
done <<< "$VALIDATOR_ASSERTIONS"

exit 0
