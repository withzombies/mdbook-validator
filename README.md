# mdbook-validator

> **Status: Design Phase** - This tool doesn't exist yet. This document describes what we're building.

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
- **Hidden context lines** - Show partial configs while validating complete ones
- **Output assertions** - Verify row counts, check for specific content
- **Expected output matching** - Regression testing for deterministic queries
- **Clean output** - All validation markers stripped from rendered documentation

## Installation

*Not yet available. Once implemented:*

```bash
cargo install mdbook-validator
```

Requires Docker to be running.

## Quick Start

1. Add to your `book.toml`:

```toml
[preprocessor.validator]
command = "mdbook-validator"

[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
validate-command = "/validators/validate-sqlite.sh"
```

2. Write validated examples in your markdown:

````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE users (id INTEGER, name TEXT);
INSERT INTO users VALUES (1, 'alice'), (2, 'bob');
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

| Marker | Purpose |
|--------|---------|
| `<!--SETUP-->` | Setup code run before the visible content (validator-interpreted) |
| `<!--ASSERT-->` | Output validation rules |
| `<!--EXPECT-->` | Exact output matching (JSON) |

### Line Prefix: `@@`

Lines starting with `@@` are sent to the validator but hidden from readers. Use this to show only relevant portions of a config while validating the complete file.

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
CREATE TABLE orders (id INTEGER, total REAL, status TEXT);
INSERT INTO orders VALUES (1, 99.99, 'shipped'), (2, 149.50, 'pending');
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
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1), (2), (3);
-->
SELECT COUNT(*) as total FROM test
<!--EXPECT
[{"total": 3}]
-->
```
````

### Skip Validation

````markdown
```sql validator=sqlite skip
-- This intentionally broken example shows what NOT to do
SELECT * FROM nonexistent_table;
```
````

## Assertions

| Assertion | Example | Description |
|-----------|---------|-------------|
| `rows = N` | `rows = 5` | Exact row count |
| `rows >= N` | `rows >= 1` | Minimum row count |
| `contains "str"` | `contains "alice"` | Output contains string |
| `matches "regex"` | `matches "user.*"` | Regex pattern match |
| `exit_code = N` | `exit_code = 0` | Script exit code |
| `file_exists` | `file_exists /etc/app.conf` | File was created |
| `stdout_contains` | `stdout_contains "success"` | Stdout has text |

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
validate-command = "/validators/validate-sqlite.sh"

# osquery SQL validator
[preprocessor.validator.validators.osquery]
container = "osquery/osquery:5.12.1-ubuntu22.04"
validate-command = "/validators/validate-osquery.sh"

# osquery config validator (JSON, not TOML!)
[preprocessor.validator.validators.osquery-config]
container = "osquery/osquery:5.12.1-ubuntu22.04"
validate-command = "/validators/validate-osquery-config.sh"

# ShellCheck static analysis
[preprocessor.validator.validators.shellcheck]
container = "koalaman/shellcheck-alpine:stable"
validate-command = "/validators/validate-shellcheck.sh"

# Bash execution with assertions
[preprocessor.validator.validators.bash-exec]
container = "ubuntu:22.04"
validate-command = "/validators/validate-bash-exec.sh"
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
validate-command = "/validate.sh"
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
validate-command = "/validate.sh"
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
validate-command = "/validate.sh"
```

## Writing Custom Validators

Validators are shell scripts that receive JSON via stdin:

```json
{
  "setup": "CREATE TABLE test (id INTEGER);",
  "content": "SELECT * FROM test;",
  "assertions": "rows >= 1\ncontains \"test\"",
  "expect": null
}
```

Exit 0 for success, non-zero for failure. Write errors to stderr.

Example validator:

```bash
#!/bin/bash
set -e

INPUT=$(cat)
SETUP=$(echo "$INPUT" | jq -r '.setup // empty')
CONTENT=$(echo "$INPUT" | jq -r '.content')
ASSERTIONS=$(echo "$INPUT" | jq -r '.assertions // empty')

# Run setup if provided
if [ -n "$SETUP" ]; then
    echo "$SETUP" | sqlite3 "$DB_FILE"
fi

# Run query
OUTPUT=$(echo "$CONTENT" | sqlite3 -json "$DB_FILE")

# Check assertions
if [ -n "$ASSERTIONS" ]; then
    ROW_COUNT=$(echo "$OUTPUT" | jq 'length')
    # ... validate assertions ...
fi

exit 0
```

## Known Limitations

1. **Container startup overhead** - First validation takes 10-20 seconds per validator type
2. **No container reuse between builds** - Each `mdbook build` starts fresh containers
3. **Marker collision** - If your code contains `-->`, it may break marker parsing
4. **No line numbers in errors** - Error messages show file but not exact line

## How It Will Work

1. mdBook calls the preprocessor with chapter content
2. Preprocessor finds code blocks with `validator=` attribute
3. Extracts markers (`<!--SETUP-->`, `<!--ASSERT-->`, `<!--EXPECT-->`) and `@@` lines
4. Starts the specified container via testcontainers
5. Runs the validator script with extracted content as JSON
6. On success: strips all markers, returns clean content to mdBook
7. On failure: exits with error, build fails

## License

Apache2

## Contributing

Contributions welcome! Please open an issue to discuss before submitting large changes.
