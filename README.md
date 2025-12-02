# mdbook-validator

[![Crates.io](https://img.shields.io/crates/v/mdbook-validator.svg)](https://crates.io/crates/mdbook-validator)
[![Documentation](https://docs.rs/mdbook-validator/badge.svg)](https://docs.rs/mdbook-validator)
[![CI](https://github.com/withzombies/mdbook-validator/actions/workflows/ci.yml/badge.svg)](https://github.com/withzombies/mdbook-validator/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/withzombies/a0277ecc8a69526d47c694467b3bf9a4/raw/coverage.json)](https://github.com/withzombies/mdbook-validator/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](https://github.com/withzombies/mdbook-validator/blob/main/LICENSE)
[![Rust Version](https://img.shields.io/badge/rust-1.75%2B-blue.svg)](https://www.rust-lang.org)

An mdBook preprocessor that validates code examples against live Docker containers during documentation builds. Catch documentation drift before it reaches your users.

## The Problem

Documentation code examples rot:
- SQL queries reference tables that were renamed
- Config files have typos that were never tested
- Examples break when the tool updates
- Code runs but produces wrong output

You only find out when a user complains.

## The Solution

`mdbook-validator` validates your code examples against real tools during `mdbook build`. If an example doesn't work, your build fails—just like a broken test.

**Key insight**: Documentation examples often need setup code (CREATE TABLE, test data) or surrounding context (full config file) that readers don't need to see. This tool lets you include that context for validation while showing only the relevant portion to readers.

## Features

- **Container-based validation** - Run examples against real tools (osquery, SQLite, etc.)
- **Hidden setup blocks** - Include setup code that's validated but not shown to readers
- **Hidden context lines** - Show partial configs while validating complete ones (`@@` prefix)
- **Hidden code blocks** - Validate entire blocks without showing them to readers (`hidden` attribute)
- **Output assertions** - Verify row counts, check for specific content
- **Expected output matching** - Regression testing for deterministic queries
- **Clean output** - All validation markers stripped from rendered documentation

## Installation

```bash
# From crates.io (once published)
cargo install mdbook-validator

# From source
cargo install --git https://github.com/withzombies/mdbook-validator
```

**Requirements:**
- Docker running (containers provide validation environments)
- `jq` installed on host (used by validator scripts for JSON parsing)

## Quick Start

1. Add to your `book.toml`:

```toml
[preprocessor.validator]
command = "mdbook-validator"

[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
script = "validators/validate-sqlite.sh"
```

2. Write validated examples in your markdown:

````markdown
```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db "CREATE TABLE users (id INTEGER, name TEXT); INSERT INTO users VALUES (1, 'alice'), (2, 'bob');"
-->
SELECT name FROM users WHERE id = 1;
<!--ASSERT
rows = 1
contains "alice"
-->
```
````

3. Build your book:

```bash
mdbook build
```

**Reader sees:**
```sql
SELECT name FROM users WHERE id = 1;
```

**Validator tests:** Complete query with setup and assertions.

## Markers

### Block Markers

| Marker | Purpose | Runs? |
|--------|---------|-------|
| `<!--SETUP-->` | Shell commands to prepare state (create tables, trigger events, write files) | **Yes** - in container via `sh -c` |
| `<!--ASSERT-->` | Output validation rules (row counts, string matching) | No - passed to validator script |
| `<!--EXPECT-->` | Exact output matching for regression testing | No - passed to validator script |

### Line Prefix: `@@`

**Important:** `@@` does NOT execute anything. It only controls what readers see.

Lines starting with `@@` are:
- ✅ **Included** in content sent to container for validation
- ❌ **Hidden** from rendered documentation output

Use this to validate complete configs while showing only the relevant portion to readers.

````markdown
```toml validator=config-check
@@base_path = "/var/data"
@@log_level = "info"
@@
[feature]
enabled = true
max_items = 100
@@
@@[advanced]
@@timeout_secs = 30
```
````

**Reader sees:**
```toml
[feature]
enabled = true
max_items = 100
```

**Validator receives:** Complete, valid config.

## Examples

### SQLite with Setup

````markdown
```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db "CREATE TABLE orders (id INTEGER, total REAL, status TEXT); INSERT INTO orders VALUES (1, 99.99, 'shipped'), (2, 149.50, 'pending');"
-->
SELECT status, COUNT(*) as count FROM orders GROUP BY status;
<!--ASSERT
rows = 2
contains "shipped"
-->
```
````

### osquery (validates against real system)

````markdown
```sql validator=osquery
SELECT uid, username FROM users WHERE username = 'root'
<!--ASSERT
rows >= 1
contains "root"
-->
```
````

### osquery Config (JSON)

````markdown
```json validator=osquery-config
{
  "options": {
    "logger_path": "/var/log/osquery",
    "disable_events": false
  },
  "schedule": {
    "system_info": {
      "query": "SELECT * FROM system_info;",
      "interval": 3600
    }
  }
}
```
````

### Expected Output (Regression Testing)

````markdown
```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db "CREATE TABLE test (id INTEGER); INSERT INTO test VALUES (1), (2), (3);"
-->
SELECT COUNT(*) as total FROM test
<!--EXPECT
[{"total": 3}]
-->
```
````

### Bash Script Execution

Validate bash scripts run correctly and produce expected results:

````markdown
```bash validator=bash-exec
#!/bin/bash
echo "Hello from bash"
exit 0
```
````

Scripts must exit 0 by default. Use `exit_code` assertion for non-zero:

````markdown
```bash validator=bash-exec
exit 42
<!--ASSERT
exit_code = 42
-->
```
````

Check file creation and content:

````markdown
```bash validator=bash-exec
mkdir -p /tmp/myapp
echo "config=value" > /tmp/myapp/settings.conf
<!--ASSERT
dir_exists /tmp/myapp
file_exists /tmp/myapp/settings.conf
file_contains /tmp/myapp/settings.conf "config=value"
stdout_contains ""
-->
```
````

### Custom Container with Plugin (Advanced)

For validating custom osquery plugins or extensions, use a custom Docker image with SETUP to trigger events:

**1. Create Dockerfile with your plugin:**
```dockerfile
FROM osquery/osquery:5.17.0-ubuntu22.04
COPY my-plugin.ext /usr/local/lib/osquery/
RUN echo "/usr/local/lib/osquery/my-plugin.ext" >> /etc/osquery/extensions.load
```

**2. Configure in book.toml:**
```toml
[preprocessor.validator.validators.my-plugin]
container = "my-osquery-plugin:latest"
script = "validators/validate-osquery.sh"
```

**3. Write validated examples with SETUP:**
````markdown
```sql validator=my-plugin
<!--SETUP
# Trigger event that populates your plugin's table
curl -X POST http://localhost:8080/trigger-event
sleep 1
-->
SELECT * FROM my_plugin_events WHERE event_type = 'login';
<!--ASSERT
rows >= 1
contains "login"
-->
```
````

**Execution flow:**
1. Container starts with your plugin loaded
2. SETUP runs `curl` and `sleep` (in container, via `sh -c`)
3. Query runs against your plugin's table (in container)
4. JSON output goes to validator script (on host)
5. Assertions checked, pass/fail returned

### Skip Validation

````markdown
```sql validator=sqlite skip
-- This intentionally broken example shows what NOT to do
SELECT * FROM nonexistent_table;
```
````

### Hidden Blocks

Use `hidden` to validate a code block without showing it to readers. The entire code fence is removed from output.

````markdown
```sql validator=sqlite hidden
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE users (id INTEGER, name TEXT);'
-->
INSERT INTO users VALUES (1, 'alice'), (2, 'bob');
```

```sql validator=sqlite
SELECT name FROM users WHERE id = 1;
<!--ASSERT
rows = 1
contains "alice"
-->
```
````

**Reader sees only:**
```sql
SELECT name FROM users WHERE id = 1;
```

The hidden block populates data that the visible query depends on. Both are validated, but only the second appears in documentation.

**Use cases:**
- Setup queries that create test data for subsequent examples
- Teardown or cleanup blocks
- Validation-only examples that shouldn't appear in docs
- Multi-step workflows where only the final step matters to readers

**Note:** `hidden` and `skip` are mutually exclusive. Using both produces error E011.

## Assertions

### SQL Validators (osquery, sqlite)

| Assertion | Example | Description |
|-----------|---------|-------------|
| `rows = N` | `rows = 5` | Exact row count |
| `rows >= N` | `rows >= 1` | Minimum row count |
| `contains "str"` | `contains "alice"` | Output contains string |
| `matches "regex"` | `matches "user.*"` | Regex pattern match |

### Bash Execution (bash-exec)

| Assertion | Example | Description |
|-----------|---------|-------------|
| `exit_code = N` | `exit_code = 0` | Script must exit with code N (default: 0) |
| `stdout_contains "str"` | `stdout_contains "success"` | Stdout must contain string |
| `file_exists /path` | `file_exists /tmp/config` | File must exist after script |
| `dir_exists /path` | `dir_exists /tmp/mydir` | Directory must exist after script |
| `file_contains /path "str"` | `file_contains /tmp/cfg "key=val"` | File must contain string |

## Configuration

```toml
[book]
title = "My Documentation"

[preprocessor.validator]
command = "mdbook-validator"
fail-fast = true  # Stop on first failure (default: true)

# SQLite validator
[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
script = "validators/validate-sqlite.sh"

# osquery SQL validator
[preprocessor.validator.validators.osquery]
container = "osquery/osquery:5.17.0-ubuntu22.04"
script = "validators/validate-osquery.sh"

# osquery config validator (JSON, not TOML!)
[preprocessor.validator.validators.osquery-config]
container = "osquery/osquery:5.17.0-ubuntu22.04"
script = "validators/validate-osquery-config.sh"

# ShellCheck static analysis
[preprocessor.validator.validators.shellcheck]
container = "koalaman/shellcheck-alpine:stable"
script = "validators/validate-shellcheck.sh"

# Bash execution with assertions
[preprocessor.validator.validators.bash-exec]
container = "ubuntu:22.04"
script = "validators/validate-bash-exec.sh"

# Python syntax validation
[preprocessor.validator.validators.python]
container = "python:3.12-slim"
script = "validators/validate-python.sh"
```

## Custom Docker Images

You can use locally-built or private registry images without pushing to a public registry.

### Local Images

Build once, reference by name:

```bash
# Build your custom validator image
docker build -t my-validator:latest validators/myvalidator/
```

```toml
[preprocessor.validator.validators.custom]
container = "my-validator:latest"  # Local image, no registry needed
script = "validators/validate-custom.sh"
```

testcontainers-rs uses local images if they exist, no pulling required.

### Private Registry

For team sharing:

```bash
docker push registry.mycompany.com/my-validator:latest
```

```toml
[preprocessor.validator.validators.custom]
container = "registry.mycompany.com/my-validator:latest"
script = "validators/validate-custom.sh"
```

Docker uses your logged-in credentials (`docker login`).

### Example: pyproject.toml Validator

`validators/pyproject/Dockerfile`:
```dockerfile
FROM python:3.12-slim-bookworm
RUN pip install --no-cache-dir 'validate-pyproject[all]' jq
COPY validate.sh /validate.sh
RUN chmod +x /validate.sh
```

`validators/pyproject/validate.sh`:
```bash
#!/bin/bash
set -e
INPUT=$(cat)
CONTENT=$(echo "$INPUT" | jq -r '.content')
TMPFILE=$(mktemp --suffix=.toml)
echo "$CONTENT" > "$TMPFILE"
validate-pyproject "$TMPFILE"
```

Build and use:
```bash
docker build -t pyproject-validator:latest validators/pyproject/
```

```toml
[preprocessor.validator.validators.pyproject]
container = "pyproject-validator:latest"
script = "validators/validate-custom.sh"
```

## Writing Custom Validators

Validators are shell scripts that run on the **host** (not in containers). They receive:

- **stdin**: JSON output from the container execution (e.g., `[{"id": 1, "name": "test"}]`)
- **VALIDATOR_ASSERTIONS** env var: Assertion rules, newline-separated
- **VALIDATOR_EXPECT** env var: Expected output for exact matching (optional)
- **CONTAINER_STDERR** env var: stderr from container execution (for warning detection)

The preprocessor handles SETUP and query execution in the container—validators only validate the output.

Exit 0 for success, non-zero for failure. Write errors to stderr.

Example validator:

```bash
#!/bin/bash
set -e

# Read JSON output from container (stdin)
JSON_OUTPUT=$(cat)

# Validate JSON is parseable
echo "$JSON_OUTPUT" | jq empty 2>/dev/null || {
    echo "Invalid JSON output" >&2
    exit 1
}

# Check assertions if provided
if [ -n "${VALIDATOR_ASSERTIONS:-}" ]; then
    ROW_COUNT=$(echo "$JSON_OUTPUT" | jq 'length')

    # Example: check "rows >= N"
    if [[ "$VALIDATOR_ASSERTIONS" == *"rows >= "* ]]; then
        expected=$(echo "$VALIDATOR_ASSERTIONS" | grep -oP 'rows >= \K\d+')
        if [ "$ROW_COUNT" -lt "$expected" ]; then
            echo "Assertion failed: rows >= $expected (got $ROW_COUNT)" >&2
            exit 1
        fi
    fi
fi

# Check expected output if provided
if [ -n "${VALIDATOR_EXPECT:-}" ]; then
    actual=$(echo "$JSON_OUTPUT" | jq -c '.')
    expected=$(echo "$VALIDATOR_EXPECT" | jq -c '.')
    if [ "$actual" != "$expected" ]; then
        echo "Output mismatch: expected $expected, got $actual" >&2
        exit 1
    fi
fi

exit 0
```

See `validators/validate-template.sh` for a comprehensive template with all assertion patterns.

## Known Limitations

1. **Container startup overhead** - First validation takes 10-20 seconds per validator type
2. **No container reuse between builds** - Each `mdbook build` starts fresh containers
3. **Marker collision** - If your code contains `-->`, it may break marker parsing
4. **No line numbers in errors** - Error messages show file but not exact line

## Execution Model

Understanding where things run is critical for writing effective validations:

```
┌─────────────────────────────────────────────────────────────────────┐
│                           HOST MACHINE                              │
│                                                                     │
│  ┌──────────────────┐                      ┌─────────────────────┐  │
│  │  mdbook-validator │                      │  Validator Script   │  │
│  │  (preprocessor)   │                      │  (e.g., validate-   │  │
│  │                   │                      │   osquery.sh)       │  │
│  │  1. Parse markdown│                      │                     │  │
│  │  2. Extract blocks│                      │  7. Receive JSON    │  │
│  │  3. Start container                      │  8. Check assertions│  │
│  └────────┬──────────┘                      │  9. Exit 0 or fail  │  │
│           │                                 └──────────▲──────────┘  │
│           │                                            │             │
│           ▼                                            │             │
│  ┌────────────────────────────────────────────────────┼──────────┐  │
│  │                    DOCKER CONTAINER                 │          │  │
│  │                                                     │          │  │
│  │   4. Run SETUP via `sh -c "<setup content>"`        │          │  │
│  │      (CREATE TABLE, trigger events, etc.)           │          │  │
│  │                                                     │          │  │
│  │   5. Run main code via `exec_command`               │          │  │
│  │      (osqueryi --json, sqlite3 -json, etc.)    ─────┘          │  │
│  │                                                JSON stdout     │  │
│  │   6. Capture stdout → send to validator                        │  │
│  │                                                                │  │
│  └────────────────────────────────────────────────────────────────┘  │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### What Runs Where

| Component | Runs In | Purpose |
|-----------|---------|---------|
| `<!--SETUP-->` content | **Container** via `sh -c` | Prepare state (create tables, trigger events, write files) |
| Main code block | **Container** via `exec_command` | Execute the query/script being documented |
| Validator script | **Host** | Validate the JSON output from container |
| `jq` (for JSON parsing) | **Host** | Used by validator scripts |

### Execution Order

1. **SETUP** (if present) → Runs first, in container, via `sh -c "<setup content>"`
2. **Main code** → Runs second, in container, via configured `exec_command`
3. **Validator** → Runs last, on host, receives container's stdout

### Common Confusion: `@@` vs `<!--SETUP-->`

These serve **completely different purposes**:

| Feature | `@@` prefix | `<!--SETUP-->` |
|---------|-------------|----------------|
| Purpose | **Hide lines** from rendered output | **Execute commands** before main code |
| Runs? | No - it's just content filtering | Yes - runs in container via `sh -c` |
| Use case | Show partial config, validate full config | Create tables, trigger events, prepare state |

**Example - `@@` hides context lines:**
````markdown
```json validator=osquery-config
@@{
@@  "options": { "disable_events": false },
@@  "schedule": {
    "my_query": {
      "query": "SELECT * FROM processes;",
      "interval": 60
    }
@@  }
@@}
```
````
Reader sees only `my_query` section. Validator receives complete JSON.

**Example - `<!--SETUP-->` prepares state:**
````markdown
```sql validator=osquery
<!--SETUP
touch /tmp/test-file.txt
-->
SELECT * FROM file WHERE path = '/tmp/test-file.txt';
<!--ASSERT
rows >= 1
-->
```
````
SETUP creates the file. Query runs after. Validator checks the result.

## How It Works

1. mdBook calls the preprocessor with chapter content
2. Preprocessor finds code blocks with `validator=` attribute
3. Extracts markers (`<!--SETUP-->`, `<!--ASSERT-->`, `<!--EXPECT-->`) and `@@` lines
4. Starts the specified container via testcontainers
5. Runs SETUP content in container via `sh -c` (if present)
6. Runs the visible content (plus `@@` lines) via `exec_command` in container
7. Captures container stdout (JSON) and stderr
8. Runs validator script **on host** with:
   - stdin: JSON output from container
   - `VALIDATOR_ASSERTIONS`: assertion rules
   - `VALIDATOR_EXPECT`: expected output
   - `VALIDATOR_CONTAINER_STDERR`: container stderr
9. On success: strips all markers and `@@` lines, returns clean content to mdBook
10. On failure: exits with error, build fails

## License

Apache2

## Contributing

Contributions welcome! Please open an issue to discuss before submitting large changes.
