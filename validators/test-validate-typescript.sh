#!/bin/bash
#
# test-validate-typescript.sh - Test suite for validate-typescript.sh
#
# Run: ./validators/test-validate-typescript.sh
# All tests should pass for validate-typescript.sh to be considered complete.
#

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VALIDATOR="$SCRIPT_DIR/validate-typescript.sh"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m' # No Color

PASSED=0
FAILED=0

# Test helper function
run_test() {
    local name="$1"
    local input="$2"
    local assertions="${3:-}"
    local expected_exit="$4"
    local expected_stderr_pattern="${5:-}"

    local actual_exit=0
    local stderr_output

    if [ -n "$assertions" ]; then
        stderr_output=$(echo "$input" | VALIDATOR_ASSERTIONS="$assertions" "$VALIDATOR" 2>&1) || actual_exit=$?
    else
        stderr_output=$(echo "$input" | "$VALIDATOR" 2>&1) || actual_exit=$?
    fi

    # Check exit code
    if [ "$actual_exit" -ne "$expected_exit" ]; then
        echo -e "${RED}FAIL${NC}: $name"
        echo "  Expected exit: $expected_exit, got: $actual_exit"
        echo "  stderr: $stderr_output"
        ((FAILED++))
        return 1
    fi

    # Check stderr pattern if provided
    if [ -n "$expected_stderr_pattern" ]; then
        if ! echo "$stderr_output" | grep -qF "$expected_stderr_pattern"; then
            echo -e "${RED}FAIL${NC}: $name"
            echo "  Expected stderr to contain: $expected_stderr_pattern"
            echo "  Actual stderr: $stderr_output"
            ((FAILED++))
            return 1
        fi
    fi

    echo -e "${GREEN}PASS${NC}: $name"
    ((PASSED++))
    return 0
}

echo "Running validate-typescript.sh tests..."
echo "========================================"

# Check validator exists
if [ ! -x "$VALIDATOR" ]; then
    echo -e "${RED}ERROR${NC}: Validator not found or not executable: $VALIDATOR"
    exit 1
fi

# Test 1: Valid execution, exit_code = 0 assertion
run_test \
    "Test 1: Valid execution with exit_code = 0 assertion" \
    '{"exit_code": 0, "stdout": "Hello World", "stderr": ""}' \
    "exit_code = 0" \
    0

# Test 2: Type error detected in stderr (compilation failed)
run_test \
    "Test 2: Type error in stderr causes failure" \
    '{"exit_code": 1, "stdout": "", "stderr": "error TS2322: Type string is not assignable"}' \
    "" \
    1 \
    "exit code 1"

# Test 3: Runtime error (execution failed after compilation)
run_test \
    "Test 3: Runtime error causes failure" \
    '{"exit_code": 1, "stdout": "", "stderr": "TypeError: Cannot read property"}' \
    "" \
    1 \
    "exit code 1"

# Test 4: contains assertion matches stdout
run_test \
    "Test 4: contains assertion matches" \
    '{"exit_code": 0, "stdout": "Hello World", "stderr": ""}' \
    'contains "Hello"' \
    0

# Test 5: contains assertion does NOT match
run_test \
    "Test 5: contains assertion fails when not found" \
    '{"exit_code": 0, "stdout": "Goodbye", "stderr": ""}' \
    'contains "Hello"' \
    1 \
    'contains "Hello": not found'

# Test 6: No assertions, default to exit_code = 0 check (success case)
run_test \
    "Test 6: No assertions, exit_code=0 succeeds" \
    '{"exit_code": 0, "stdout": "output", "stderr": ""}' \
    "" \
    0

# Test 7: No assertions, script failed
run_test \
    "Test 7: No assertions, exit_code=1 fails" \
    '{"exit_code": 1, "stdout": "", "stderr": "error"}' \
    "" \
    1 \
    "exit code 1"

# Test 8: Malformed JSON input
run_test \
    "Test 8: Malformed JSON rejected" \
    'not json' \
    "" \
    1 \
    "Invalid JSON"

# Test 9: Missing fields in JSON (stdout missing) - should treat as empty
run_test \
    "Test 9: Missing stdout field treated as empty" \
    '{"exit_code": 0}' \
    "" \
    0

# Test 10: stdout_contains assertion (explicit stdout check)
run_test \
    "Test 10: stdout_contains assertion works" \
    '{"exit_code": 0, "stdout": "value is 42", "stderr": ""}' \
    'stdout_contains "42"' \
    0

# Test 11: rows assertion for JSON array output
run_test \
    "Test 11: rows assertion for JSON array in stdout" \
    '{"exit_code": 0, "stdout": "[{\"a\":1},{\"b\":2}]", "stderr": ""}' \
    'rows = 2' \
    0

# Test 12: rows assertion fails when count wrong
run_test \
    "Test 12: rows assertion fails on wrong count" \
    '{"exit_code": 0, "stdout": "[{\"a\":1}]", "stderr": ""}' \
    'rows = 5' \
    1 \
    "rows = 5"

# Test 13: exit_code assertion with non-zero expected
run_test \
    "Test 13: exit_code = 1 assertion matches" \
    '{"exit_code": 1, "stdout": "", "stderr": "expected error"}' \
    'exit_code = 1' \
    0

# Test 14: Unknown assertion syntax
run_test \
    "Test 14: Unknown assertion rejected" \
    '{"exit_code": 0, "stdout": "", "stderr": ""}' \
    'unknown_assertion foo' \
    1 \
    "Unknown assertion"

echo ""
echo "========================================"
echo -e "Results: ${GREEN}$PASSED passed${NC}, ${RED}$FAILED failed${NC}"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
exit 0
