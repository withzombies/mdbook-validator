#!/bin/sh
#
# validate-sqlite.sh - Validate SQLite SQL queries.
#
# Reads from environment variables:
# - VALIDATOR_CONTENT: SQL query to execute (required)
# - VALIDATOR_SETUP: Setup SQL to run first, without JSON output (optional)
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

# Create temp database file
DB_FILE=$(mktemp)
trap 'rm -f "$DB_FILE"' EXIT

# Run SETUP if provided (without -json flag)
if [ -n "${VALIDATOR_SETUP:-}" ]; then
    SETUP_ERR=$(printf '%s\n' "$VALIDATOR_SETUP" | sqlite3 "$DB_FILE" 2>&1) || {
        echo "Setup SQL failed: $SETUP_ERR" >&2
        exit 1
    }
fi

# Run query with JSON output
OUTPUT=$(printf '%s\n' "$VALIDATOR_CONTENT" | sqlite3 -json "$DB_FILE" 2>&1) || {
    echo "Query failed: $OUTPUT" >&2
    exit 1
}

# Count rows by counting opening braces that start JSON objects
# SQLite JSON output format: [{"col":val},\n{"col":val}] (first row has leading [)
# Count occurrences of {"  which marks start of each row object
ROW_COUNT=$(printf '%s\n' "$OUTPUT" | grep -o '{\"' | wc -l | tr -d ' ') || ROW_COUNT=0

# Evaluate assertions if provided
if [ -n "${VALIDATOR_ASSERTIONS:-}" ]; then
    printf '%s\n' "$VALIDATOR_ASSERTIONS" | while IFS= read -r line; do
        # Skip empty lines
        [ -z "$line" ] && continue

        # rows = N
        if printf '%s\n' "$line" | grep -q '^rows[[:space:]]*=[[:space:]]*[0-9]*$'; then
            expected=$(printf '%s\n' "$line" | sed 's/^rows[[:space:]]*=[[:space:]]*\([0-9]*\)$/\1/')
            if [ "$ROW_COUNT" -ne "$expected" ]; then
                echo "Assertion failed: rows = $expected: got $ROW_COUNT" >&2
                exit 1
            fi
            continue
        fi

        # rows >= N
        if printf '%s\n' "$line" | grep -q '^rows[[:space:]]*>=[[:space:]]*[0-9]*$'; then
            expected=$(printf '%s\n' "$line" | sed 's/^rows[[:space:]]*>=[[:space:]]*\([0-9]*\)$/\1/')
            if [ "$ROW_COUNT" -lt "$expected" ]; then
                echo "Assertion failed: rows >= $expected: got $ROW_COUNT" >&2
                exit 1
            fi
            continue
        fi

        # rows > N
        if printf '%s\n' "$line" | grep -q '^rows[[:space:]]*>[[:space:]]*[0-9]*$'; then
            expected=$(printf '%s\n' "$line" | sed 's/^rows[[:space:]]*>[[:space:]]*\([0-9]*\)$/\1/')
            if [ "$ROW_COUNT" -le "$expected" ]; then
                echo "Assertion failed: rows > $expected: got $ROW_COUNT" >&2
                exit 1
            fi
            continue
        fi

        # contains "string"
        if printf '%s\n' "$line" | grep -q '^contains[[:space:]]*"'; then
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
    normalized_output=$(printf '%s\n' "$OUTPUT" | tr -d '[:space:]')
    normalized_expect=$(printf '%s\n' "$VALIDATOR_EXPECT" | tr -d '[:space:]')

    if [ "$normalized_output" != "$normalized_expect" ]; then
        echo "Output mismatch:" >&2
        echo "  Expected: $VALIDATOR_EXPECT" >&2
        echo "  Actual:   $OUTPUT" >&2
        exit 1
    fi
fi

# Output the JSON result (for debugging/verification)
printf '%s\n' "$OUTPUT"
exit 0
