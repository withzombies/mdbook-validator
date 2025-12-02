#!/bin/bash
#
# validate-sqlite.sh - Host-based SQLite JSON output validator.
#
# This script validates JSON output from sqlite3 -json commands.
# It runs on the HOST (not in container) and uses jq for JSON parsing.
#
# Input: JSON via stdin (from sqlite3 -json output)
# Environment:
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
# - VALIDATOR_EXPECT: Expected JSON output for exact match (optional)
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

# If no assertions and no expected output, just verify we got valid JSON
if [ -z "${VALIDATOR_ASSERTIONS:-}" ] && [ -z "${VALIDATOR_EXPECT:-}" ]; then
    echo "$JSON_INPUT" | jq empty 2>/dev/null || {
        echo "Invalid JSON output" >&2
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
            rows\ =\ *)
                expected=${assertion#rows = }
                if ! is_integer "$expected"; then
                    echo "Assertion failed: rows = $expected: invalid integer" >&2
                    exit 1
                fi
                actual=$(echo "$JSON_INPUT" | jq 'length')
                if [ "$actual" -ne "$expected" ]; then
                    echo "Assertion failed: rows = $expected: got $actual" >&2
                    exit 1
                fi
                ;;
            rows\ \>=\ *)
                expected=${assertion#rows >= }
                if ! is_integer "$expected"; then
                    echo "Assertion failed: rows >= $expected: invalid integer" >&2
                    exit 1
                fi
                actual=$(echo "$JSON_INPUT" | jq 'length')
                if [ "$actual" -lt "$expected" ]; then
                    echo "Assertion failed: rows >= $expected: got $actual" >&2
                    exit 1
                fi
                ;;
            rows\ \>\ *)
                expected=${assertion#rows > }
                if ! is_integer "$expected"; then
                    echo "Assertion failed: rows > $expected: invalid integer" >&2
                    exit 1
                fi
                actual=$(echo "$JSON_INPUT" | jq 'length')
                if [ "$actual" -le "$expected" ]; then
                    echo "Assertion failed: rows > $expected: got $actual" >&2
                    exit 1
                fi
                ;;
            columns\ =\ *)
                expected=${assertion#columns = }
                if ! is_integer "$expected"; then
                    echo "Assertion failed: columns = $expected: invalid integer" >&2
                    exit 1
                fi
                # Handle empty array case - columns = 0 for empty results
                actual=$(echo "$JSON_INPUT" | jq 'if length == 0 then 0 else (.[0] | keys | length) end')
                if [ "$actual" -ne "$expected" ]; then
                    echo "Assertion failed: columns = $expected: got $actual" >&2
                    exit 1
                fi
                ;;
            contains\ *)
                needle=${assertion#contains }
                # Remove surrounding quotes if present
                needle=${needle#\"}
                needle=${needle%\"}
                if ! echo "$JSON_INPUT" | jq -e --arg s "$needle" 'any(.. | strings; contains($s))' >/dev/null 2>&1; then
                    echo "Assertion failed: contains \"$needle\": not found in output" >&2
                    exit 1
                fi
                ;;
            *)
                echo "Assertion failed: Unknown assertion syntax: $assertion" >&2
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
