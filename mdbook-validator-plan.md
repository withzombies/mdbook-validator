# mdbook-validator Project Plan

## Project Overview

Build an mdBook preprocessor that validates code examples against live containers during documentation builds, catching documentation drift before it reaches users.

**Key insight**: Documentation code examples rot. SQL queries reference non-existent tables, configs have typos, examples break when the tool updates. This preprocessor validates examples against real tools, ensuring documentation stays accurate.

The first implementation targets osquery SQL validation and osquery JSON config validation, ensuring all code examples in documentation execute successfully against the actual tool.

## Goals

1. Create a Rust-based mdBook preprocessor that intercepts code blocks with validation annotations
2. Use testcontainers-rs to spin up validation environments on-demand
3. Validate SQL queries execute against real osquery (proves docs match tool)
4. Validate JSON configs parse correctly with osquery's config checker
5. Support hidden setup blocks that run before visible content (validator interprets appropriately)
6. Support output assertions and expected results for deterministic validators
7. Strip validation-only markers before passing clean examples to mdBook for rendering
8. Fail the build early if any examples fail execution or validation
9. Make the system extensible for other tools/languages

## Non-Goals (v1)

- Showing command output in the rendered book (that's mdbook-cmdrun's job)
- Supporting non-container validation (though architecture should allow it later)
- Web UI for validation results
- Performance optimization (correctness first)
- Reusable setup blocks in book.toml (inline only for v1)

## Marker System (Simplified)

### Block Markers (stripped from output)

| Marker | Purpose | Validator Interpretation |
|--------|---------|-------------------------|
| `<!--SETUP-->` | Pre-query content | Validator-specific (SQL setup, bash commands, etc.) |
| `<!--ASSERT-->` | Output validation rules | Row counts, contains, patterns |
| `<!--EXPECT-->` | Exact output matching | JSON comparison for regression tests |

**Design principle**: SETUP content meaning is validator-specific. The preprocessor just extracts it; the validator script decides what to do with it.

### Line Prefix: `@@` (hidden context lines)

Lines starting with `@@` are:
1. Sent to the validator (with `@@` stripped)
2. Removed from rendered output

**Use case**: Show only relevant portions of a config while validating the complete file.

```toml validator=dlp-config
@@watch_paths = ["/home/%%"]
@@exclude_paths = []
@@
[policies]
enabled_policies = ["ccpa"]

# This comment will appear in docs
[policies.policy_configs.ccpa]
enabled = true
settings = { confidence_threshold = "0.7" }

@@[work_queue]
@@max_queue_size = 10000
@@submit_timeout_secs = 5
@@
@@[worker]
@@num_workers = 0
```

**Reader sees:**
```toml
[policies]
enabled_policies = ["ccpa"]

# This comment will appear in docs
[policies.policy_configs.ccpa]
enabled = true
settings = { confidence_threshold = "0.7" }
```

**Validator receives** (complete, valid TOML):
```toml
watch_paths = ["/home/%%"]
exclude_paths = []

[policies]
enabled_policies = ["ccpa"]

# This comment will appear in docs
[policies.policy_configs.ccpa]
enabled = true
settings = { confidence_threshold = "0.7" }

[work_queue]
max_queue_size = 10000
submit_timeout_secs = 5

[worker]
num_workers = 0
```

**Key benefit**: Language-agnostic. Works with TOML, JSON, YAML, SQL, or any format.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ mdBook Build Process                                         │
│                                                              │
│  ┌──────────────┐        ┌──────────────────┐              │
│  │   Markdown   │───────>│  mdbook-validator │              │
│  │    Files     │        │   Preprocessor    │              │
│  └──────────────┘        └─────────┬─────────┘              │
│                                    │                         │
│                                    v                         │
│                          ┌──────────────────┐               │
│                          │ Parse code blocks │               │
│                          │ with validator=   │               │
│                          │   annotations     │               │
│                          └─────────┬─────────┘               │
│                                    │                         │
│                                    v                         │
│                          ┌──────────────────┐               │
│                          │ Extract markers: │               │
│                          │ - SETUP          │               │
│                          │ - ASSERT         │               │
│                          │ - EXPECT         │               │
│                          └─────────┬─────────┘               │
│                                    │                         │
│                                    v                         │
│                          ┌──────────────────┐               │
│                          │ testcontainers-rs│               │
│                          │ Start container  │               │
│                          └─────────┬─────────┘               │
│                                    │                         │
│                                    v                         │
│                          ┌──────────────────┐               │
│                          │  Run validator   │               │
│                          │  via sh -c "..." │               │
│                          │  (shell wrapper) │               │
│                          └─────────┬─────────┘               │
│                                    │                         │
│                          ┌─────────v─────────┐              │
│                          │ Validate output   │              │
│                          │ - Check assertions│              │
│                          │ - Compare expect  │              │
│                          └─────────┬─────────┘              │
│                                    │                         │
│                          ┌─────────v─────────┐              │
│                     PASS │ Strip all markers │              │
│                          │ Return clean code │              │
│                          │ to mdBook         │              │
│                          └───────────────────┘              │
│                                    │                         │
│                          ┌─────────v─────────┐              │
│                     FAIL │   Exit build      │              │
│                          │   with error      │              │
│                          └───────────────────┘              │
└─────────────────────────────────────────────────────────────┘
```

## Tech Stack

- **Language**: Rust (2021 edition)
- **Core Dependencies**:
  - `mdbook` - preprocessor interface
  - `testcontainers` - container orchestration (with `blocking` feature)
  - `pulldown-cmark` - markdown parsing
  - `pulldown-cmark-to-cmark` - markdown reconstruction
  - `serde`, `serde_json` - config and data handling
  - `anyhow` - error handling
  - `tracing` - logging
- **Containers** (specific tags, NOT :latest):
  - `osquery/osquery:5.12.1-ubuntu22.04` - osquery SQL and config validation
  - `python:3.12-slim-bookworm` - pyproject.toml validation (with validate-pyproject pre-installed)
  - `koalaman/shellcheck-alpine:stable` - shell script static analysis (Alpine variant has shell)
  - `ubuntu:22.04` - shell script execution with assertions
  - `keinos/sqlite3:3.47.2` - SQLite validation with setup support
- **Test Framework**: `cargo test` with integration tests

## Project Structure

```
mdbook-validator/
├── Cargo.toml
├── README.md
├── CLAUDE.md                  # Project context for Claude
├── src/
│   ├── main.rs               # CLI entry point
│   ├── lib.rs                # Library interface
│   ├── preprocessor.rs       # Main preprocessor logic
│   ├── parser.rs             # Markdown parsing and marker extraction
│   ├── transpiler.rs         # Strip markers from validated blocks
│   ├── validator.rs          # Validation orchestration
│   ├── container.rs          # Container management
│   └── config.rs             # Configuration handling
├── tests/
│   ├── integration_tests.rs
│   └── fixtures/
│       ├── test-book/
│       │   ├── book.toml
│       │   └── src/
│       │       ├── SUMMARY.md
│       │       ├── osquery-examples.md
│       │       ├── pyproject-examples.md
│       │       ├── shell-examples.md
│       │       └── sqlite-examples.md
│       └── validators/
│           ├── validate-osquery.sh
│           ├── validate-osquery-config.sh
│           ├── validate-pyproject.sh
│           ├── validate-shellcheck.sh
│           ├── validate-bash-exec.sh
│           └── validate-sqlite.sh
├── examples/
│   └── security-docs/
│       ├── book.toml
│       └── src/
└── validators/                # Production validator scripts
    ├── validate-osquery.sh
    ├── validate-osquery-config.sh
    ├── validate-pyproject.sh
    ├── validate-shellcheck.sh
    ├── validate-bash-exec.sh
    └── validate-sqlite.sh
```

## Implementation Phases

### Phase 1: Core Preprocessor (MVP)
**Goal**: Get basic validation working for osquery SQL

- [ ] Set up Rust project structure with Cargo.toml
- [ ] Implement mdBook preprocessor trait (stdin JSON -> stdout JSON)
- [ ] Parse markdown with pulldown-cmark, find code blocks with `validator=` attribute
- [ ] Extract SETUP, ASSERT, EXPECT markers from code block content
- [ ] Start osquery container with testcontainers-rs (blocking feature)
- [ ] Execute validator script via `sh -c "..."` wrapper (required for piping)
- [ ] Create JSON input: `{setup, content, assertions, expect}`
- [ ] Pass if validator exits 0, fail if non-zero
- [ ] Strip all markers and return clean content to mdBook on success
- [ ] Write integration test with test book

**Critical implementation detail**: testcontainers-rs `exec()` doesn't support shell piping. All validator invocations must use:
```rust
container.exec(ExecCommand::new(["sh", "-c", "cat /tmp/input.json | /validators/validate-osquery.sh"]))
```

**Success Criteria**: Can validate osquery SQL blocks, build fails on invalid SQL or schema errors

### Phase 1b: SQLite Validator
**Goal**: Add SQLite validation with setup blocks

- [ ] Add SQLite validator configuration
- [ ] Implement setup block handling in validator script:
  - Run SETUP content as SQL first (CREATE/INSERT)
  - Run visible content as the query to validate
  - This solves sqlite3's "multiple SELECT = invalid JSON" problem
- [ ] Validate output against assertions
- [ ] Validate output against expected JSON
- [ ] Strip all markers from output

**SQLite validator approach** (solves multiple-SELECT issue):
```bash
# Run setup SQL separately (no JSON output needed)
if [ -n "$SETUP" ]; then
    echo "$SETUP" | sqlite3 "$DB_FILE"
fi
# Run query and capture JSON output
OUTPUT=$(echo "$CONTENT" | sqlite3 -json "$DB_FILE")
```

**Success Criteria**: Can validate SQLite blocks with setup and assertions

### Phase 2: osquery Config Validation
**Goal**: Add JSON config validation for osquery

**IMPORTANT**: osquery configs are JSON, not TOML! From osquery docs:
> "By default, osqueryd will look for a JSON file on disk... The filesystem plugin architecture expects config plugins to yield valid JSON."

- [ ] Create JSON config validator script
- [ ] Test with osquery config files using `osqueryd --config_check`
- [ ] Add pyproject.toml validator (this one IS TOML, validated by validate-pyproject)

**Success Criteria**: Can validate osquery JSON configs and Python pyproject.toml files

### Phase 3: Shell Script Validators
**Goal**: Add ShellCheck and bash execution validators

- [ ] ShellCheck validator using `koalaman/shellcheck-alpine:stable` (NOT the scratch-based image)
- [ ] Bash execution validator with post-execution assertions
- [ ] Support assertions: exit_code, file_exists, stdout_contains, etc.

**Container note**: The base `koalaman/shellcheck` image is scratch-based with NO shell. Must use `shellcheck-alpine` variant which includes ash/bash.

**Success Criteria**: Can validate shell scripts with both static analysis and execution

### Phase 4: Better Error Messages & DX
**Goal**: Make validation failures helpful

- [ ] Show file and approximate line number (via pulldown-cmark offset tracking)
- [ ] Display visible content and setup context in errors
- [ ] Display validator stderr output clearly
- [ ] Show which assertions failed with expected vs actual values
- [ ] Show output diff when EXPECT doesn't match
- [ ] Add "skip" annotation for intentionally broken examples
- [ ] Add dry-run mode
- [ ] Validate marker syntax (error on unclosed markers)

**Success Criteria**: When validation fails, users immediately know what's wrong and how to fix it

### Phase 5: Performance & Reliability
**Goal**: Make builds reasonably fast

**Realistic expectations**:
- Container startup: 10-20 seconds per validator type (not 2-5 as originally estimated)
- testcontainers-rs Issue #742 (container reuse) is still open as of late 2024
- Target: 50 validations in < 3 minutes (not 60 seconds)

- [ ] Keep container handles alive for entire build via struct field (not OnceLock alone)
- [ ] Add benchmark suite to track performance
- [ ] Evaluate bollard for direct container management if needed
- [ ] Consider "external container mode" where user pre-starts containers

**Success Criteria**: Builds complete in reasonable time for books with <100 validated blocks

## Code Block Annotation Syntax

The preprocessor looks for info strings with `validator=` attributes.

### Basic Syntax
```
```language validator=name [skip]
```

### osquery Examples

**Basic query** (validates syntax and schema against real osquery):
````markdown
```sql validator=osquery
SELECT uid, username, shell FROM users LIMIT 5;
```
````

**With assertions**:
````markdown
```sql validator=osquery
SELECT uid, username FROM users WHERE username = 'root'
<!--ASSERT
rows >= 1
contains "root"
-->
```
````

**Skip validation** (for intentionally broken examples):
````markdown
```sql validator=osquery skip
-- This example shows what NOT to do
SELECT * FROM nonexistent_table;
```
````

### osquery Config (JSON, not TOML!)

````markdown
```json validator=osquery-config
{
  "options": {
    "logger_path": "/var/log/osquery",
    "config_plugin": "filesystem"
  },
  "packs": {
    "incident_response": "/etc/osquery/packs/ir.conf"
  }
}
```
````

### SQLite with Setup

````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE alerts (path TEXT, scanner TEXT);
INSERT INTO alerts VALUES ('/data/test.json', 'scanner1');
-->
SELECT path FROM alerts WHERE path LIKE '%.json';
<!--ASSERT
rows >= 1
contains "test.json"
-->
```
````

Reader sees only: `SELECT path FROM alerts WHERE path LIKE '%.json';`

### SQLite with Expected Output (Regression Test)

````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1), (2), (3);
-->
SELECT id FROM test ORDER BY id
<!--EXPECT
[{"id": 1}, {"id": 2}, {"id": 3}]
-->
```
````

### pyproject.toml Validation

````markdown
```toml validator=pyproject
[project]
name = "my-package"
version = "1.0.0"
requires-python = ">=3.8"
dependencies = ["requests>=2.28"]

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"
```
````

### Shell Script - Static Analysis (ShellCheck)

````markdown
```bash validator=shellcheck
#!/bin/bash
set -euo pipefail

for file in "$@"; do
    if [[ -f "$file" ]]; then
        echo "Processing: $file"
        cat "$file"
    fi
done
```
````

### Shell Script - Execution with Assertions

````markdown
```bash validator=bash-exec
#!/bin/bash
set -e

mkdir -p /etc/myapp
echo "port = 8080" > /etc/myapp/config.toml
echo "Configuration installed"
<!--ASSERT
exit_code = 0
dir_exists /etc/myapp
file_exists /etc/myapp/config.toml
file_contains /etc/myapp/config.toml "port = 8080"
stdout_contains "installed"
-->
```
````

## Validator Script Interface

Validator scripts receive JSON via stdin and return exit code 0 for success.

### JSON Input Format

```json
{
  "setup": "CREATE TABLE test (id INTEGER);\nINSERT INTO test VALUES (1);",
  "content": "SELECT * FROM test;",
  "assertions": "rows >= 1\ncontains \"test\"",
  "expect": null
}
```

### Validator: osquery SQL

```bash
#!/bin/bash
# validate-osquery.sh
set -e

INPUT=$(cat)
CONTENT=$(echo "$INPUT" | jq -r '.content')
ASSERTIONS=$(echo "$INPUT" | jq -r '.assertions // empty')

# Execute query - note: must use stdin pipe for osqueryi
OUTPUT=$(echo "$CONTENT" | osqueryi --json 2>&1)
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo "Query execution failed:" >&2
    echo "$OUTPUT" >&2
    exit 1
fi

# Check assertions if provided
if [ -n "$ASSERTIONS" ]; then
    ROW_COUNT=$(echo "$OUTPUT" | jq 'length')

    # rows >= N
    if echo "$ASSERTIONS" | grep -qE "^rows >= [0-9]+"; then
        MIN=$(echo "$ASSERTIONS" | grep -E "^rows >= " | awk '{print $3}')
        if [ "$ROW_COUNT" -lt "$MIN" ]; then
            echo "Assertion failed: rows >= $MIN (got $ROW_COUNT)" >&2
            exit 1
        fi
    fi

    # contains "string"
    while IFS= read -r line; do
        if [[ "$line" =~ ^contains[[:space:]]+\"(.*)\"$ ]]; then
            SEARCH="${BASH_REMATCH[1]}"
            if ! echo "$OUTPUT" | grep -q "$SEARCH"; then
                echo "Assertion failed: output doesn't contain '$SEARCH'" >&2
                exit 1
            fi
        fi
    done <<< "$ASSERTIONS"
fi

echo "Query executed successfully, returned $ROW_COUNT rows"
exit 0
```

### Validator: osquery Config (JSON)

```bash
#!/bin/bash
# validate-osquery-config.sh
set -e

INPUT=$(cat)
CONFIG=$(echo "$INPUT" | jq -r '.content')

TMPFILE=$(mktemp --suffix=.conf)
echo "$CONFIG" > "$TMPFILE"
trap "rm -f $TMPFILE" EXIT

# Validate with osquery config checker
osqueryd --config_path="$TMPFILE" --config_check --verbose 2>&1
exit $?
```

### Validator: SQLite (with setup)

```bash
#!/bin/bash
# validate-sqlite.sh
set -e

INPUT=$(cat)
SETUP=$(echo "$INPUT" | jq -r '.setup // empty')
CONTENT=$(echo "$INPUT" | jq -r '.content')
ASSERTIONS=$(echo "$INPUT" | jq -r '.assertions // empty')
EXPECT=$(echo "$INPUT" | jq -r '.expect // empty')

DB_FILE=$(mktemp --suffix=.db)
trap "rm -f $DB_FILE" EXIT

# Run setup SQL separately (solves multiple-SELECT JSON issue)
if [ -n "$SETUP" ]; then
    echo "$SETUP" | sqlite3 "$DB_FILE" 2>&1
    if [ $? -ne 0 ]; then
        echo "Setup SQL failed" >&2
        exit 1
    fi
fi

# Run query and capture JSON output
OUTPUT=$(echo "$CONTENT" | sqlite3 -json "$DB_FILE" 2>&1)
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    echo "Query execution failed:" >&2
    echo "$OUTPUT" >&2
    exit 1
fi

# Validate assertions if provided
if [ -n "$ASSERTIONS" ]; then
    ROW_COUNT=$(echo "$OUTPUT" | jq 'length')

    # rows = N
    if echo "$ASSERTIONS" | grep -qE "^rows = [0-9]+"; then
        EXPECTED=$(echo "$ASSERTIONS" | grep -E "^rows = " | awk '{print $3}')
        if [ "$ROW_COUNT" -ne "$EXPECTED" ]; then
            echo "Assertion failed: rows = $EXPECTED (got $ROW_COUNT)" >&2
            exit 1
        fi
    fi

    # rows >= N
    if echo "$ASSERTIONS" | grep -qE "^rows >= [0-9]+"; then
        MIN=$(echo "$ASSERTIONS" | grep -E "^rows >= " | awk '{print $3}')
        if [ "$ROW_COUNT" -lt "$MIN" ]; then
            echo "Assertion failed: rows >= $MIN (got $ROW_COUNT)" >&2
            exit 1
        fi
    fi

    # contains "string"
    while IFS= read -r line; do
        if [[ "$line" =~ ^contains[[:space:]]+\"(.*)\"$ ]]; then
            SEARCH="${BASH_REMATCH[1]}"
            if ! echo "$OUTPUT" | grep -q "$SEARCH"; then
                echo "Assertion failed: output doesn't contain '$SEARCH'" >&2
                exit 1
            fi
        fi
    done <<< "$ASSERTIONS"
fi

# Validate expected output if provided
if [ -n "$EXPECT" ]; then
    NORMALIZED_OUTPUT=$(echo "$OUTPUT" | jq -S '.')
    NORMALIZED_EXPECT=$(echo "$EXPECT" | jq -S '.')

    if [ "$NORMALIZED_OUTPUT" != "$NORMALIZED_EXPECT" ]; then
        echo "Output mismatch:" >&2
        echo "Expected:" >&2
        echo "$NORMALIZED_EXPECT" >&2
        echo "Actual:" >&2
        echo "$NORMALIZED_OUTPUT" >&2
        exit 1
    fi
fi

echo "Validation passed"
exit 0
```

### Validator: pyproject.toml

```bash
#!/bin/bash
# validate-pyproject.sh
set -e

INPUT=$(cat)
CONTENT=$(echo "$INPUT" | jq -r '.content')

TMPFILE=$(mktemp --suffix=.toml)
echo "$CONTENT" > "$TMPFILE"
trap "rm -f $TMPFILE" EXIT

# validate-pyproject should be pre-installed in container image
validate-pyproject "$TMPFILE"
```

**Container requirement**: Build custom image with validate-pyproject pre-installed:
```dockerfile
FROM python:3.12-slim-bookworm
RUN pip install --no-cache-dir 'validate-pyproject[all]' jq
```

### Validator: ShellCheck

```bash
#!/bin/bash
# validate-shellcheck.sh
set -e

INPUT=$(cat)
CONTENT=$(echo "$INPUT" | jq -r '.content')

TMPFILE=$(mktemp --suffix=.sh)
echo "$CONTENT" > "$TMPFILE"
trap "rm -f $TMPFILE" EXIT

shellcheck -s bash "$TMPFILE"
```

**Container requirement**: Must use `koalaman/shellcheck-alpine:stable`, NOT the scratch-based image.

### Validator: Bash Execution

```bash
#!/bin/bash
# validate-bash-exec.sh
set -e

INPUT=$(cat)
SETUP=$(echo "$INPUT" | jq -r '.setup // empty')
CONTENT=$(echo "$INPUT" | jq -r '.content')
ASSERTIONS=$(echo "$INPUT" | jq -r '.assertions // empty')

# Run setup if provided
if [ -n "$SETUP" ]; then
    eval "$SETUP"
fi

TMPFILE=$(mktemp --suffix=.sh)
echo "$CONTENT" > "$TMPFILE"
chmod +x "$TMPFILE"

STDOUT=$(mktemp)
STDERR=$(mktemp)
trap "rm -f $TMPFILE $STDOUT $STDERR" EXIT

set +e
bash "$TMPFILE" > "$STDOUT" 2> "$STDERR"
ACTUAL_EXIT_CODE=$?
set -e

# Check assertions
if [ -n "$ASSERTIONS" ]; then
    # exit_code = N
    if echo "$ASSERTIONS" | grep -qE "^exit_code = [0-9]+"; then
        EXPECTED=$(echo "$ASSERTIONS" | grep -E "^exit_code = " | awk '{print $3}')
        if [ "$ACTUAL_EXIT_CODE" -ne "$EXPECTED" ]; then
            echo "Assertion failed: exit_code = $EXPECTED (got $ACTUAL_EXIT_CODE)" >&2
            cat "$STDERR" >&2
            exit 1
        fi
    fi

    # file_exists /path
    while IFS= read -r line; do
        if [[ "$line" =~ ^file_exists[[:space:]]+(.+)$ ]]; then
            FILEPATH="${BASH_REMATCH[1]}"
            if [ ! -f "$FILEPATH" ]; then
                echo "Assertion failed: file_exists $FILEPATH" >&2
                exit 1
            fi
        fi
    done <<< "$ASSERTIONS"

    # dir_exists /path
    while IFS= read -r line; do
        if [[ "$line" =~ ^dir_exists[[:space:]]+(.+)$ ]]; then
            DIRPATH="${BASH_REMATCH[1]}"
            if [ ! -d "$DIRPATH" ]; then
                echo "Assertion failed: dir_exists $DIRPATH" >&2
                exit 1
            fi
        fi
    done <<< "$ASSERTIONS"

    # stdout_contains "string"
    while IFS= read -r line; do
        if [[ "$line" =~ ^stdout_contains[[:space:]]+\"(.+)\"$ ]]; then
            SEARCH="${BASH_REMATCH[1]}"
            if ! grep -q "$SEARCH" "$STDOUT"; then
                echo "Assertion failed: stdout_contains \"$SEARCH\"" >&2
                cat "$STDOUT" >&2
                exit 1
            fi
        fi
    done <<< "$ASSERTIONS"

    # file_contains /path "pattern"
    while IFS= read -r line; do
        if [[ "$line" =~ ^file_contains[[:space:]]+([^[:space:]]+)[[:space:]]+\"(.+)\"$ ]]; then
            FILEPATH="${BASH_REMATCH[1]}"
            PATTERN="${BASH_REMATCH[2]}"
            if [ ! -f "$FILEPATH" ]; then
                echo "Assertion failed: file_contains - file not found: $FILEPATH" >&2
                exit 1
            fi
            if ! grep -q "$PATTERN" "$FILEPATH"; then
                echo "Assertion failed: file_contains $FILEPATH \"$PATTERN\"" >&2
                exit 1
            fi
        fi
    done <<< "$ASSERTIONS"
fi

# Default: require exit code 0 if no exit_code assertion
if ! echo "$ASSERTIONS" | grep -q "^exit_code"; then
    if [ "$ACTUAL_EXIT_CODE" -ne 0 ]; then
        echo "Script failed with exit code $ACTUAL_EXIT_CODE" >&2
        cat "$STDERR" >&2
        exit 1
    fi
fi

echo "Script executed successfully"
exit 0
```

## Assertion Syntax Reference

| Assertion | Example | Description |
|-----------|---------|-------------|
| `rows = N` | `rows = 5` | Exact row count |
| `rows >= N` | `rows >= 1` | Minimum row count |
| `rows > N` | `rows > 0` | Greater than |
| `columns = N` | `columns = 3` | Column count |
| `contains "str"` | `contains "alice"` | Output contains string |
| `matches "regex"` | `matches "user.*"` | Regex pattern match |
| `json_valid` | `json_valid` | Output is valid JSON |
| `exit_code = N` | `exit_code = 0` | Script exit code |
| `file_exists` | `file_exists /etc/config` | File was created |
| `dir_exists` | `dir_exists /var/log/app` | Directory was created |
| `stdout_contains` | `stdout_contains "success"` | Stdout has text |
| `file_contains` | `file_contains /etc/hosts "localhost"` | File has pattern |

## Configuration (book.toml)

```toml
[book]
title = "Security Documentation"
authors = ["Your Name"]

[preprocessor.validator]
command = "mdbook-validator"
fail-fast = true

# Validators - use specific tags, NOT :latest
[preprocessor.validator.validators.osquery]
container = "osquery/osquery:5.12.1-ubuntu22.04"
validate-command = "/validators/validate-osquery.sh"

[preprocessor.validator.validators.osquery-config]
container = "osquery/osquery:5.12.1-ubuntu22.04"
validate-command = "/validators/validate-osquery-config.sh"

[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
validate-command = "/validators/validate-sqlite.sh"

[preprocessor.validator.validators.pyproject]
container = "mdbook-validator/python-validate:3.12"  # Custom image with validate-pyproject
validate-command = "/validators/validate-pyproject.sh"

[preprocessor.validator.validators.shellcheck]
container = "koalaman/shellcheck-alpine:stable"  # Alpine variant has shell!
validate-command = "/validators/validate-shellcheck.sh"

[preprocessor.validator.validators.bash-exec]
container = "ubuntu:22.04"
validate-command = "/validators/validate-bash-exec.sh"
```

## Error Handling

### Example Error Messages

**Schema drift (osquery updated)**:
```
Error: Validation failed in src/network-queries.md

  | ```sql validator=osquery
  | SELECT local_port, remote_address FROM listening_ports;
  | ```

Validator output:
  Error: no such table: listening_ports

The table 'listening_ports' doesn't exist in osquery 5.12.1.

Possible causes:
  - Table was renamed in a recent osquery version
  - Table is platform-specific (check osquery schema)
  - Typo in table name

See: https://osquery.io/schema/5.12.1/
```

**Assertion failure**:
```
Error: Assertion failed in src/examples.md

  | ```sql validator=sqlite
  | <!--SETUP
  | CREATE TABLE users (id INTEGER, name TEXT);
  | INSERT INTO users VALUES (1, 'alice'), (2, 'bob');
  | -->
  | SELECT COUNT(*) as total FROM users
  | <!--ASSERT
  | total = 5
  | -->
  | ```

Assertion details:
  total = 5
  Expected: 5
  Actual:   2

Query output:
  [{"total": 2}]
```

**osquery config error** (JSON, not TOML):
```
Error: Validation failed in src/config.md

  | ```json validator=osquery-config
  | {
  |   "options": {
  |     "logger_path": "/var/log/osquery"
  |   }
  | }

Validator output:
  Error: Error reading config: Invalid JSON

Note: osquery configs must be valid JSON, not TOML.
```

## Testing Strategy

### Unit Tests
- Parser correctly extracts code blocks with annotations
- Marker extraction handles edge cases (nested quotes, etc.)
- Config parsing handles all valid configurations

### Integration Tests
- Full preprocessor run against test books
- Valid SQL passes validation
- Invalid SQL fails validation
- Valid JSON config passes
- Invalid JSON config fails
- Setup blocks execute before visible content
- Assertions validate correctly
- EXPECT matches trigger on mismatch

### Manual Testing
- Run against real osquery documentation
- Test with no Docker available (clear error message)

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Block markers | SETUP/ASSERT/EXPECT only | SETUP is validator-interpreted (SQL for sqlite, bash for others) |
| Hidden lines | `@@` prefix | Language-agnostic; show partial configs while validating complete ones |
| osquery config format | JSON | osquery requires JSON, not TOML |
| Container tags | Specific versions | `:latest` is unpredictable |
| Shell wrapper | `sh -c "..."` | testcontainers exec() doesn't support piping |
| ShellCheck container | Alpine variant | Scratch image has no shell |
| SQLite multiple SELECT | Run SETUP separately | sqlite3 -json produces invalid JSON with multiple SELECTs |
| Performance target | ~3 min for 50 blocks | Container startup is 10-20s, not 2-5s |
| Reusable setups | Not in v1 | Keep it simple; inline everything |

## Known Limitations

1. **No container reuse**: testcontainers-rs Issue #742 is still open. Each build starts fresh containers.

2. **Windows stdin pipe bug**: osqueryi has a known bug (#7972) where stdin piping fails on Windows with "incomplete SQL" errors.

3. **Marker collision**: If your SQL contains `-->`, it will break marker parsing. Use unique markers in config if needed.

4. **No line numbers**: Error messages show file but not exact line numbers (would require offset-to-line mapping).

## Success Metrics

For v1 release, we've succeeded if:

1. osquery SQL queries validate against real osquery (catches schema drift)
2. osquery JSON configs validate with config checker
3. pyproject.toml validates against PEP standards
4. Shell scripts pass ShellCheck analysis
5. Shell scripts run and pass execution assertions
6. SQLite queries work with setup and assertions
7. Clear error messages show what failed and why
8. Zero false positives
9. Build fails when docs don't match tool behavior
10. At least one external project adopts it

## Resources

- [mdBook preprocessor docs](https://rust-lang.github.io/mdBook/for_developers/preprocessors.html)
- [testcontainers-rs docs](https://docs.rs/testcontainers/latest/testcontainers/)
- [pulldown-cmark docs](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/)
- [pulldown-cmark-to-cmark docs](https://docs.rs/pulldown-cmark-to-cmark/latest/)
- [osquery schema](https://osquery.io/schema/)
- [osquery configuration (JSON)](https://osquery.readthedocs.io/en/stable/deployment/configuration/)
- [validate-pyproject](https://pypi.org/project/validate-pyproject/)
- [testcontainers-rs Issue #742 (container reuse)](https://github.com/testcontainers/testcontainers-rs/issues/742)

## Getting Started

```bash
# Create the project
cargo new mdbook-validator --bin
cd mdbook-validator

# Add dependencies
cargo add mdbook anyhow serde serde_json tracing tracing-subscriber
cargo add pulldown-cmark pulldown-cmark-to-cmark
cargo add testcontainers --features=blocking

# Create initial structure
mkdir -p src tests/fixtures validators

# Start implementing src/main.rs
```

## Initial Implementation Starter

### src/main.rs
```rust
use anyhow::Result;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use std::io;

fn main() -> Result<()> {
    let preprocessor = mdbook_validator::ValidatorPreprocessor::new()?;

    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    if ctx.mdbook_version != mdbook::MDBOOK_VERSION {
        eprintln!(
            "Warning: mdbook version mismatch. Expected {}, got {}",
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = preprocessor.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}
```

### src/lib.rs
```rust
pub mod preprocessor;
pub mod parser;
pub mod transpiler;
pub mod validator;
pub mod container;
pub mod config;

pub use preprocessor::ValidatorPreprocessor;
```

### src/parser.rs (starter)
```rust
use anyhow::{anyhow, Result};

/// The hidden line prefix - lines starting with this are sent to validator
/// but removed from rendered output
const HIDDEN_LINE_PREFIX: &str = "@@";

pub struct CodeBlock {
    pub language: String,
    pub validator: Option<String>,
    /// Content shown to readers (no @@ lines, no markers)
    pub visible_content: String,
    /// Full content for validation (@@ prefix stripped, markers extracted)
    pub validation_content: String,
    pub setup: Option<String>,
    pub assertions: Option<String>,
    pub expected_output: Option<String>,
    pub skip: bool,
}

pub struct MarkerConfig {
    pub setup_start: String,
    pub setup_end: String,
    pub assert_start: String,
    pub assert_end: String,
    pub expect_start: String,
    pub expect_end: String,
}

impl Default for MarkerConfig {
    fn default() -> Self {
        Self {
            setup_start: "<!--SETUP".to_string(),
            setup_end: "-->".to_string(),
            assert_start: "<!--ASSERT".to_string(),
            assert_end: "-->".to_string(),
            expect_start: "<!--EXPECT".to_string(),
            expect_end: "-->".to_string(),
        }
    }
}

/// Process @@ hidden lines: returns (visible_lines, all_lines_with_prefix_stripped)
fn process_hidden_lines(content: &str) -> (String, String) {
    let mut visible_lines = Vec::new();
    let mut all_lines = Vec::new();

    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix(HIDDEN_LINE_PREFIX) {
            // Hidden line: include in validation (stripped), exclude from visible
            all_lines.push(stripped);
        } else {
            // Visible line: include in both
            visible_lines.push(line);
            all_lines.push(line);
        }
    }

    (visible_lines.join("\n"), all_lines.join("\n"))
}

/// Extract content between marker_start and marker_end
fn extract_marker(
    content: &str,
    marker_start: &str,
    marker_end: &str,
) -> Result<(String, Option<String>)> {
    let Some(start_idx) = content.find(marker_start) else {
        return Ok((content.to_string(), None));
    };

    let after_start = &content[start_idx + marker_start.len()..];
    let end_idx = after_start
        .find(marker_end)
        .ok_or_else(|| anyhow!(
            "Unclosed marker: found '{}' without matching '{}'",
            marker_start,
            marker_end
        ))?;

    let marker_content = after_start[..end_idx].trim().to_string();
    let remaining = format!(
        "{}{}",
        &content[..start_idx],
        &after_start[end_idx + marker_end.len()..]
    );

    Ok((remaining.trim().to_string(), Some(marker_content)))
}

impl CodeBlock {
    pub fn parse(
        info_string: &str,
        content: &str,
        markers: &MarkerConfig,
    ) -> Result<Self> {
        let (language, validator, skip) = parse_info_string(info_string);

        // Extract block markers in order
        let (after_setup, setup) = extract_marker(
            content,
            &markers.setup_start,
            &markers.setup_end,
        )?;

        let (after_assert, assertions) = extract_marker(
            &after_setup,
            &markers.assert_start,
            &markers.assert_end,
        )?;

        let (after_expect, expected_output) = extract_marker(
            &after_assert,
            &markers.expect_start,
            &markers.expect_end,
        )?;

        // Process @@ hidden lines
        // visible_content: what readers see (no @@ lines)
        // validation_content: what validator receives (@@ prefix stripped)
        let (visible_content, validation_content) = process_hidden_lines(&after_expect);

        Ok(CodeBlock {
            language,
            validator,
            visible_content,
            validation_content,
            setup,
            assertions,
            expected_output,
            skip,
        })
    }
}

fn parse_info_string(info: &str) -> (String, Option<String>, bool) {
    let parts: Vec<&str> = info.split_whitespace().collect();
    let language = parts.first().map(|s| s.to_string()).unwrap_or_default();

    let mut validator = None;
    let mut skip = false;

    for part in &parts[1..] {
        if let Some(v) = part.strip_prefix("validator=") {
            validator = Some(v.to_string());
        }
        if *part == "skip" {
            skip = true;
        }
    }

    (language, validator, skip)
}
```

---

## Summary

Key design decisions:

1. **Three block markers**: SETUP, ASSERT, EXPECT - SETUP is validator-interpreted
2. **`@@` line prefix** - Hide context lines while validating complete content
3. **osquery configs are JSON** - NOT TOML (osquery requires JSON)
4. **Specific container tags** - No `:latest` (e.g., `osquery/osquery:5.12.1-ubuntu22.04`)
5. **Shell wrapper required** - `sh -c "..."` for all container exec
6. **ShellCheck container** - Must use Alpine variant (scratch image has no shell)
7. **SQLite** - Run SETUP separately from query to avoid invalid JSON
8. **Realistic performance** - 10-20s container startup, ~3 min for 50 blocks
9. **Inline setup only (v1)** - No reusable setup blocks from book.toml
