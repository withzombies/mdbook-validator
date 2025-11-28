#!/bin/bash
#
# validate-osquery-config.sh - Host-based osquery config validator.
#
# This script validates osquery config JSON that has been checked by
# osqueryi --config_check in the container. It runs on the HOST (not
# in container) and uses jq for JSON parsing.
#
# Input: Config JSON via stdin (echoed back after osqueryi --config_check passed)
# Environment:
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
# - VALIDATOR_EXPECT: Expected JSON output for exact match (optional)
#
# Exits 0 on success, 1 on failure with details to stderr.
#

set -e

# Check jq is available
command -v jq >/dev/null 2>&1 || {
    echo "ERROR: jq is required but not installed" >&2
    exit 1
}

# Read JSON from stdin
JSON_INPUT=$(cat)

# If no assertions and no expected output, just verify we got valid JSON
if [ -z "${VALIDATOR_ASSERTIONS:-}" ] && [ -z "${VALIDATOR_EXPECT:-}" ]; then
    echo "$JSON_INPUT" | jq empty 2>/dev/null || {
        echo "Invalid JSON config" >&2
        exit 1
    }
    exit 0
fi

# Evaluate assertions if provided
if [ -n "${VALIDATOR_ASSERTIONS:-}" ]; then
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
                # Check if the string appears anywhere in the JSON (keys or values)
                if ! echo "$JSON_INPUT" | jq -e --arg s "$needle" 'any(.. | strings; contains($s))' >/dev/null 2>&1; then
                    # Also check if it appears as a key name
                    if ! echo "$JSON_INPUT" | jq -e --arg s "$needle" '[.. | objects | keys[]] | any(contains($s))' >/dev/null 2>&1; then
                        echo "Assertion failed: contains \"$needle\": not found in config" >&2
                        exit 1
                    fi
                fi
                ;;
            *)
                echo "Assertion failed: Unknown assertion syntax: $assertion" >&2
                echo "Supported assertions for config validation: contains \"string\"" >&2
                exit 1
                ;;
        esac
    done <<< "$VALIDATOR_ASSERTIONS"
fi

# Check expected output if provided
if [ -n "${VALIDATOR_EXPECT:-}" ]; then
    # Normalize both outputs for comparison (remove whitespace differences)
    normalized_output=$(echo "$JSON_INPUT" | jq -c '.' 2>/dev/null || echo "$JSON_INPUT" | tr -d '[:space:]')
    normalized_expect=$(echo "$VALIDATOR_EXPECT" | jq -c '.' 2>/dev/null || echo "$VALIDATOR_EXPECT" | tr -d '[:space:]')

    if [ "$normalized_output" != "$normalized_expect" ]; then
        echo "Output mismatch:" >&2
        echo "  Expected: $VALIDATOR_EXPECT" >&2
        echo "  Actual:   $JSON_INPUT" >&2
        exit 1
    fi
fi

exit 0
