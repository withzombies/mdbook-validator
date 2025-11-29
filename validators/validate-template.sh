#!/bin/bash
#
# =============================================================================
# VALIDATOR TEMPLATE - mdbook-validator
# =============================================================================
#
# PURPOSE
# -------
# This is a template for creating new validator scripts for mdbook-validator.
# Validators run on the HOST (not inside containers) and validate output from
# container-executed commands.
#
# Copy this file, rename it (e.g., validate-myvalidator.sh), and customize
# the validation logic for your use case.
#
# ARCHITECTURE OVERVIEW
# ---------------------
# mdbook-validator uses host-based validation:
#
#   1. Code block content → Container (runs tool, produces output)
#   2. Container stdout → Validator stdin (this script)
#   3. Container stderr → VALIDATOR_CONTAINER_STDERR env var
#   4. Validator checks output → Exit 0 (pass) or non-zero (fail)
#
# This separation keeps validators simple and gives you access to host tools
# like jq, grep, awk, etc.
#
# =============================================================================
# ENVIRONMENT VARIABLES
# =============================================================================
#
# VALIDATOR_CONTAINER_STDERR (optional)
#   Contains stderr output from the container command.
#   Useful for catching warnings or errors that tools write to stderr.
#   Example: shellcheck findings, Python SyntaxErrors, osquery warnings
#
# VALIDATOR_ASSERTIONS (optional)
#   Newline-separated assertion rules from <!--ASSERT--> blocks.
#   Common formats:
#     rows = N        - Exact row count (JSON arrays)
#     rows >= N       - Minimum row count
#     rows > N        - Greater than row count
#     columns = N     - Column count (first row of JSON array)
#     contains "str"  - String appears in output
#   Parse with: while IFS= read -r assertion; do ... done <<< "$VALIDATOR_ASSERTIONS"
#
# VALIDATOR_EXPECT (optional)
#   Expected output from <!--EXPECT--> blocks for exact matching.
#   Useful for regression testing where output should be deterministic.
#   Compare normalized versions to ignore whitespace differences.
#
# =============================================================================
# INPUT/OUTPUT CONTRACT
# =============================================================================
#
# Input:  Container stdout via stdin (e.g., JSON from sqlite3 -json)
# Output: Exit 0 = validation passed
#         Exit non-zero = validation failed (write details to stderr)
#
# Error messages should be written to stderr and explain:
#   - What check failed
#   - What was expected
#   - What was actually received
#
# =============================================================================
# EXAMPLE BOOK.TOML CONFIGURATION
# =============================================================================
#
# [preprocessor.validator.validators.myvalidator]
# container = "myimage:1.0.0"      # Specific tag, NEVER :latest
# script = "validators/validate-myvalidator.sh"
# # Content is passed via stdin - use cat to read it:
# exec_command = "sh -c 'cat > /tmp/input.txt && mycommand /tmp/input.txt'"
# # Or for tools that read stdin natively (like sqlite3):
# exec_command = "mycommand --json"
#
# =============================================================================

set -e

# -----------------------------------------------------------------------------
# PATTERN 1: Read stdin (container stdout)
# -----------------------------------------------------------------------------
# Always read stdin first. This is the output from the container command.
OUTPUT=$(cat)

# -----------------------------------------------------------------------------
# PATTERN 2: Check container stderr for errors (optional)
# -----------------------------------------------------------------------------
# Many tools write errors/warnings to stderr. Check VALIDATOR_CONTAINER_STDERR
# for patterns that indicate failure.
#
# Examples from existing validators:
#   - shellcheck: grep -qE "(^In .* line [0-9]+:|SC[0-9]{4})"
#   - python:     grep -qE "(SyntaxError|IndentationError|TabError)"
#   - osquery:    grep -q "Cannot set unknown"
#
if [ -n "${VALIDATOR_CONTAINER_STDERR:-}" ]; then
    # CUSTOMIZE: Add patterns for your tool's error messages
    if echo "$VALIDATOR_CONTAINER_STDERR" | grep -qE "(ERROR|FATAL|Exception)"; then
        echo "Validation failed: errors detected in container stderr" >&2
        echo "$VALIDATOR_CONTAINER_STDERR" >&2
        exit 1
    fi
fi

# -----------------------------------------------------------------------------
# PATTERN 3: Validate JSON output (if applicable)
# -----------------------------------------------------------------------------
# If your tool outputs JSON, use jq for parsing and validation.
# Note: jq must be installed on the host.
#
# command -v jq >/dev/null 2>&1 || {
#     echo "ERROR: jq is required but not installed" >&2
#     exit 1
# }
#
# # Verify valid JSON
# echo "$OUTPUT" | jq empty 2>/dev/null || {
#     echo "Invalid JSON output" >&2
#     exit 1
# }
#
# # Get row count
# ROW_COUNT=$(echo "$OUTPUT" | jq 'length')
#
# # Check for string in JSON (any nested string value)
# echo "$OUTPUT" | jq -e --arg s "needle" 'any(.. | strings; contains($s))'

# -----------------------------------------------------------------------------
# PATTERN 4: Process assertions (if no assertions, exit early)
# -----------------------------------------------------------------------------
if [ -z "${VALIDATOR_ASSERTIONS:-}" ] && [ -z "${VALIDATOR_EXPECT:-}" ]; then
    # No assertions or expected output - basic validation passed
    exit 0
fi

# Process assertions line by line
if [ -n "${VALIDATOR_ASSERTIONS:-}" ]; then
    while IFS= read -r assertion || [ -n "$assertion" ]; do
        # Skip empty lines and trim whitespace
        assertion=$(echo "$assertion" | xargs 2>/dev/null || echo "$assertion")
        [ -z "$assertion" ] && continue

        case "$assertion" in
            # CUSTOMIZE: Add assertion types your validator supports

            rows\ =\ *)
                # Example: rows = 5
                # shellcheck disable=SC2034  # expected is for user to implement
                expected=${assertion#rows = }
                # Uncomment and customize:
                # actual=$(echo "$OUTPUT" | jq 'length')
                # if [ "$actual" -ne "$expected" ]; then
                #     echo "Assertion failed: rows = $expected: got $actual" >&2
                #     exit 1
                # fi
                echo "TODO: Implement rows = assertion for your validator" >&2
                exit 1
                ;;

            rows\ \>=\ *)
                # Example: rows >= 1
                # shellcheck disable=SC2034  # expected is for user to implement
                expected=${assertion#rows >= }
                # Uncomment and customize:
                # actual=$(echo "$OUTPUT" | jq 'length')
                # if [ "$actual" -lt "$expected" ]; then
                #     echo "Assertion failed: rows >= $expected: got $actual" >&2
                #     exit 1
                # fi
                echo "TODO: Implement rows >= assertion for your validator" >&2
                exit 1
                ;;

            contains\ *)
                # Example: contains "expected text"
                needle=${assertion#contains }
                # Remove surrounding quotes if present
                needle=${needle#\"}
                needle=${needle%\"}

                # Check stdout first, then stderr
                if ! echo "$OUTPUT" | grep -qF "$needle"; then
                    if ! echo "${VALIDATOR_CONTAINER_STDERR:-}" | grep -qF "$needle"; then
                        echo "Assertion failed: contains \"$needle\": not found in output" >&2
                        exit 1
                    fi
                fi
                ;;

            *)
                echo "Assertion failed: Unknown assertion syntax: $assertion" >&2
                echo "Supported assertions: rows = N, rows >= N, contains \"string\"" >&2
                exit 1
                ;;
        esac
    done <<< "$VALIDATOR_ASSERTIONS"
fi

# -----------------------------------------------------------------------------
# PATTERN 5: Check expected output (exact match)
# -----------------------------------------------------------------------------
if [ -n "${VALIDATOR_EXPECT:-}" ]; then
    # Normalize both outputs for comparison
    # For JSON: use jq -c to compact
    # For text: remove whitespace with tr

    # JSON normalization example:
    # normalized_output=$(echo "$OUTPUT" | jq -c '.' 2>/dev/null || echo "$OUTPUT" | tr -d '[:space:]')
    # normalized_expect=$(echo "$VALIDATOR_EXPECT" | jq -c '.' 2>/dev/null || echo "$VALIDATOR_EXPECT" | tr -d '[:space:]')

    # Text normalization example:
    normalized_output=$(echo "$OUTPUT" | tr -d '[:space:]')
    normalized_expect=$(echo "$VALIDATOR_EXPECT" | tr -d '[:space:]')

    if [ "$normalized_output" != "$normalized_expect" ]; then
        echo "Output mismatch:" >&2
        echo "  Expected: $VALIDATOR_EXPECT" >&2
        echo "  Actual:   $OUTPUT" >&2
        exit 1
    fi
fi

# -----------------------------------------------------------------------------
# All checks passed
# -----------------------------------------------------------------------------
exit 0
