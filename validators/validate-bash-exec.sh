#!/bin/bash
#
# validate-bash-exec.sh - Host-based bash script validator.
#
# This script validates bash script execution results from the container.
# Container runs script and outputs JSON: {"exit_code": N, "stdout": "...", "stderr": "..."}
# Validator parses JSON and checks assertions.
#
# Input: JSON via stdin (from container execution)
# Environment:
# - VALIDATOR_ASSERTIONS: Assertion rules, newline-separated (optional)
#   - exit_code = N: Script must exit with code N
#   - stdout_contains "string": Stdout must contain string
#   - file_exists /path: File must exist (requires files in JSON)
#   - dir_exists /path: Directory must exist (requires files in JSON)
#   - file_contains /path "string": File must contain string (requires files in JSON)
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

# Parse fields from JSON
EXIT_CODE=$(echo "$JSON_INPUT" | jq -r '.exit_code')
STDOUT=$(echo "$JSON_INPUT" | jq -r '.stdout')
STDERR=$(echo "$JSON_INPUT" | jq -r '.stderr')

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
            stdout_contains\ *)
                needle=${assertion#stdout_contains }
                # Remove surrounding quotes if present
                needle=${needle#\"}
                needle=${needle%\"}
                if ! echo "$STDOUT" | grep -qF "$needle"; then
                    echo "Assertion failed: stdout_contains \"$needle\": not found" >&2
                    echo "stdout: $STDOUT" >&2
                    exit 1
                fi
                ;;
            file_exists\ *)
                filepath=${assertion#file_exists }
                filepath=$(echo "$filepath" | xargs)
                # Check files JSON object for this path
                exists=$(echo "$JSON_INPUT" | jq -r --arg p "$filepath" '.files[$p].exists // false')
                if [ "$exists" != "true" ]; then
                    echo "Assertion failed: file_exists $filepath: file not found" >&2
                    exit 1
                fi
                ;;
            dir_exists\ *)
                dirpath=${assertion#dir_exists }
                dirpath=$(echo "$dirpath" | xargs)
                # Check files JSON object for this path
                is_dir=$(echo "$JSON_INPUT" | jq -r --arg p "$dirpath" '.files[$p].is_dir // false')
                if [ "$is_dir" != "true" ]; then
                    echo "Assertion failed: dir_exists $dirpath: directory not found" >&2
                    exit 1
                fi
                ;;
            file_contains\ *)
                # Format: file_contains /path "string"
                rest=${assertion#file_contains }
                # Extract path (everything before the first quote)
                filepath=$(echo "$rest" | sed 's/ *".*$//')
                # Extract needle (content between quotes)
                needle=$(echo "$rest" | sed 's/^[^"]*"//' | sed 's/"$//')
                # Get file content from JSON
                content=$(echo "$JSON_INPUT" | jq -r --arg p "$filepath" '.files[$p].content // ""')
                if [ -z "$content" ] || ! echo "$content" | grep -qF "$needle"; then
                    echo "Assertion failed: file_contains $filepath \"$needle\": not found" >&2
                    exit 1
                fi
                ;;
            *)
                echo "Assertion failed: Unknown assertion syntax: $assertion" >&2
                echo "Supported: exit_code = N, stdout_contains \"str\", file_exists /path, dir_exists /path, file_contains /path \"str\"" >&2
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
