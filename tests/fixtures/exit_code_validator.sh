#!/bin/sh
# Test validator that exits with a specific code (default 42)

# Exit code can be passed as first argument or defaults to 42
exit ${1:-42}
