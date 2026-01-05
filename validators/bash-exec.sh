#!/bin/sh
#
# bash-exec.sh - Container execution script for bash validation.
#
# Reads bash script from stdin, executes it, outputs JSON result.
# Used by bash-exec validator to run scripts and capture output.
#
# Output format: {"exit_code": N, "stdout": "...", "stderr": "..."}
#

# Read script from stdin
cat > /tmp/script.sh
chmod +x /tmp/script.sh

# Execute the script, capturing stdout and stderr
bash /tmp/script.sh > /tmp/stdout.txt 2> /tmp/stderr.txt
EXIT_CODE=$?

# Escape outputs for JSON (backslashes first, then quotes, remove newlines)
STDOUT=$(cat /tmp/stdout.txt | tr -d '\n' | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')
STDERR=$(cat /tmp/stderr.txt | tr -d '\n' | sed 's/\\/\\\\/g' | sed 's/"/\\"/g')

# Output JSON result
printf '{"exit_code": %d, "stdout": "%s", "stderr": "%s"}' "$EXIT_CODE" "$STDOUT" "$STDERR"
