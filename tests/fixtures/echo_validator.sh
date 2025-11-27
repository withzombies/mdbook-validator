#!/bin/sh
# Test validator that echoes stdin and env vars for verification

# Read JSON from stdin
JSON_INPUT=$(cat)

# Output for verification
echo "JSON_INPUT: $JSON_INPUT"
echo "VALIDATOR_ASSERTIONS: $VALIDATOR_ASSERTIONS"
echo "VALIDATOR_EXPECT: $VALIDATOR_EXPECT"

exit 0
