#!/bin/sh
#
# validate-osquery.sh - Validate osquery SQL against live osquery instance.
#
# Reads from environment variables:
# - VALIDATOR_CONTENT: SQL query to execute (required)
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
# - VALIDATOR_EXPECT: Expected JSON output for exact match (optional)
#
# Exits 0 on success, 1 on failure with details to stderr.
#

set -eu

# Check required content
if [ -z "${VALIDATOR_CONTENT:-}" ]; then
    echo "Query failed: VALIDATOR_CONTENT is empty" >&2
    exit 1
fi

# Run query with osqueryi
# Use echo pipe to pass SQL (heredoc not portable to POSIX sh)
OUTPUT=$(printf '%s\n' "$VALIDATOR_CONTENT" | osqueryi --json 2>&1) || {
    echo "Query failed: $OUTPUT" >&2
    exit 1
}

# Count rows by counting lines with opening braces (each row starts with {)
# osquery JSON output format: [{...}, {...}, ...]
# Use grep without -c to avoid issues, count manually
ROW_COUNT=$(echo "$OUTPUT" | grep -c '^\s*{' 2>/dev/null) || ROW_COUNT=0

# Evaluate assertions if provided
if [ -n "${VALIDATOR_ASSERTIONS:-}" ]; then
    printf '%s\n' "$VALIDATOR_ASSERTIONS" | while IFS= read -r line; do
        # Skip empty lines
        [ -z "$line" ] && continue

        # rows = N
        if printf '%s\n' "$line" | grep -q '^rows[[:space:]]*=[[:space:]]*[0-9]*$'; then
            # Extract the number using sed
            expected=$(printf '%s\n' "$line" | sed 's/^rows[[:space:]]*=[[:space:]]*\([0-9]*\)$/\1/')
            if [ "$ROW_COUNT" -ne "$expected" ]; then
                echo "Assertion failed: rows = $expected: got $ROW_COUNT" >&2
                exit 1
            fi
            continue
        fi

        # rows >= N
        if printf '%s\n' "$line" | grep -q '^rows[[:space:]]*>=[[:space:]]*[0-9]*$'; then
            # Extract the number using sed
            expected=$(printf '%s\n' "$line" | sed 's/^rows[[:space:]]*>=[[:space:]]*\([0-9]*\)$/\1/')
            if [ "$ROW_COUNT" -lt "$expected" ]; then
                echo "Assertion failed: rows >= $expected: got $ROW_COUNT" >&2
                exit 1
            fi
            continue
        fi

        # rows > N
        if printf '%s\n' "$line" | grep -q '^rows[[:space:]]*>[[:space:]]*[0-9]*$'; then
            # Extract the number using sed
            expected=$(printf '%s\n' "$line" | sed 's/^rows[[:space:]]*>[[:space:]]*\([0-9]*\)$/\1/')
            if [ "$ROW_COUNT" -le "$expected" ]; then
                echo "Assertion failed: rows > $expected: got $ROW_COUNT" >&2
                exit 1
            fi
            continue
        fi

        # contains "string"
        if printf '%s\n' "$line" | grep -q '^contains[[:space:]]*"'; then
            # Extract the string between quotes using sed
            needle=$(printf '%s\n' "$line" | sed 's/^contains[[:space:]]*"\([^"]*\)"$/\1/')
            if ! printf '%s\n' "$OUTPUT" | grep -q "$needle"; then
                echo "Assertion failed: contains \"$needle\": not found in output" >&2
                exit 1
            fi
            continue
        fi

        # Unknown assertion
        echo "Assertion failed: Unknown assertion syntax: $line" >&2
        exit 1

    done
fi

# Check expected output if provided
if [ -n "${VALIDATOR_EXPECT:-}" ]; then
    # Normalize both outputs for comparison (remove whitespace differences)
    normalized_output=$(echo "$OUTPUT" | tr -d '[:space:]')
    normalized_expect=$(echo "$VALIDATOR_EXPECT" | tr -d '[:space:]')

    if [ "$normalized_output" != "$normalized_expect" ]; then
        echo "Output mismatch:" >&2
        echo "  Expected: $VALIDATOR_EXPECT" >&2
        echo "  Actual:   $OUTPUT" >&2
        exit 1
    fi
fi

exit 0
