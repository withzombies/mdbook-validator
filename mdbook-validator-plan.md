# mdbook-validator Project Plan

## Implementation Status Summary (Updated 2025-11-27)

| Phase | Name | Status | Notes |
|-------|------|--------|-------|
| 0 | Project Setup | ✅ COMPLETE | Cargo.toml, deny.toml, hooks, all modules |
| 1 | Core Preprocessor (MVP) | ✅ COMPLETE | osquery SQL validation working |
| 1b | SQLite Validator | ✅ COMPLETE | Setup, assertions, expected output all working |
| 2 | Transpiler | ✅ COMPLETE | Marker stripping + @@ lines |
| 3 | Configuration | ✅ COMPLETE | book.toml parsing, multi-validator |
| 4 | Error Reporting | ✅ COMPLETE | Chapter/validator/exit code/stderr |
| 5 | osquery Config | ❌ NOT STARTED | JSON config validation |
| 6 | Shell Script Validators | ❌ NOT STARTED | ShellCheck + bash-exec |
| 7 | Performance | ⚡ PARTIAL | Container caching implemented |

**Key Implementation Discoveries:**
- Python not available in osquery container → using shell scripts with grep-based row counting
- Cannot build custom Docker images → using standard images only
- Using environment variables instead of stdin (simpler bollard API)
- Using runtime file loading instead of `include_bytes!` (more flexible for users)
- Assertions implemented in validator scripts, not Rust (user-customizable)
- **Host-based validation architecture**: Container runs query tool (osqueryi, sqlite3), JSON output piped to host validator script using local `jq` - simpler than installing jq in each container

**Test Coverage:** 84 tests (10 osquery, 15 sqlite, 12 parser, 8 transpiler, 9 config, 7 container, 5 host_validator, 12 integration, 4 container_image, 2 prototype)

---

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
│   ┌────────────────────────────────┼────────────────────────┐
│   │ Hybrid Container Approach      │                        │
│   │                                v                        │
│   │  ┌─────────────────────────────────────────────────┐   │
│   │  │ testcontainers-rs                                │   │
│   │  │ • Start container (GenericImage)                 │   │
│   │  │ • Copy validator script (with_copy_to)           │   │
│   │  │ • Manage lifecycle (auto-cleanup on drop)        │   │
│   │  │ • Get container ID for bollard                   │   │
│   │  └──────────────────────┬──────────────────────────┘   │
│   │                         │                               │
│   │                         v                               │
│   │  ┌─────────────────────────────────────────────────┐   │
│   │  │ bollard (via docker_client_instance())          │   │
│   │  │ • create_exec with attach_stdin: true           │   │
│   │  │ • start_exec → get input/output streams         │   │
│   │  │ • Write JSON to stdin, read stdout/stderr       │   │
│   │  │ • Get exit code via inspect_exec                │   │
│   │  └──────────────────────┬──────────────────────────┘   │
│   └─────────────────────────┼───────────────────────────────┘
│                             │                               │
│                   ┌─────────v─────────┐                    │
│                   │ Validate output   │                    │
│                   │ - Check assertions│                    │
│                   │ - Compare expect  │                    │
│                   └─────────┬─────────┘                    │
│                             │                               │
│              ┌──────────────┴──────────────┐               │
│              │                             │                │
│         PASS │                        FAIL │                │
│              v                             v                │
│    ┌─────────────────┐          ┌─────────────────┐       │
│    │ Strip markers   │          │  Exit build     │       │
│    │ Return clean    │          │  with error     │       │
│    │ code to mdBook  │          │  + diagnostics  │       │
│    └─────────────────┘          └─────────────────┘       │
└─────────────────────────────────────────────────────────────┘
```

**Why the hybrid approach?**
- testcontainers-rs `ExecCommand` does NOT support stdin attachment
- testcontainers-rs handles container lifecycle (cleanup on drop, port mapping)
- bollard provides full Docker API access including `attach_stdin`
- Both libraries use the same underlying Docker connection

## Tech Stack

- **Language**: Rust (2021 edition)
- **Core Dependencies**:
  - `mdbook_preprocessor = "0.5"` - preprocessor interface (mdBook 0.5.x split crates)
  - `testcontainers` - container lifecycle management (async only - see Async/Sync Bridging)
  - `bollard` - Docker API client for exec with env vars (testcontainers-rs uses this internally)
  - `pulldown-cmark` - markdown parsing (use `into_offset_iter()` for source spans)
  - `serde`, `serde_json` - config and data handling
  - `anyhow` - error handling
  - `tracing` - logging
  - `tokio` - async runtime (required for bollard)
  - `futures-util` - stream utilities for exec output
- **Containers** (specific tags, NOT :latest or :stable):
  - `osquery/osquery:5.17.0-ubuntu22.04` - osquery SQL and config validation
  - `python:3.12-slim-bookworm` - pyproject.toml validation (with validate-pyproject pre-installed)
  - `koalaman/shellcheck-alpine:v0.10.0` - shell script static analysis (Alpine variant has shell)
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
│   ├── validator.rs          # Container exec and output collection
│   ├── assertions.rs         # Parse and evaluate assertions in Rust
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

**STATUS: COMPLETE** (Epic: mdbook-validator-1xm, closed 2025-11-27)

- [x] Set up Rust project structure with Cargo.toml
- [x] Implement mdBook preprocessor trait (stdin JSON -> stdout JSON)
- [x] Parse markdown with pulldown-cmark, find code blocks with `validator=` attribute
- [x] Extract SETUP, ASSERT, EXPECT markers from code block content
- [x] Implement validator script packaging (runtime file loading, not embedded - see Discovery below)
- [x] Start osquery container with testcontainers-rs, copy validator script via `with_copy_to()`
- [x] Execute validator with env vars via bollard (not stdin - see Discovery below)
- [x] Pass env vars: `VALIDATOR_CONTENT`, `VALIDATOR_SETUP`, `VALIDATOR_ASSERTIONS`, `VALIDATOR_EXPECT`
- [x] Pass if validator exits 0, fail if non-zero
- [x] Strip all markers and return clean content to mdBook on success
- [x] Write integration test with test book (10 osquery tests + 12 parser + 8 transpiler + 4 config)

**Discovery Log:**
- Python not available in osquery/osquery:5.17.0-ubuntu22.04 container
- Cannot build custom Docker images per project constraints
- Using shell script (validate-osquery.sh) with grep-based row counting instead of Python
- Using environment variables instead of stdin (simpler bollard API)
- Using runtime file loading instead of `include_bytes!` (more flexible for users)

**Critical implementation detail**: testcontainers-rs + bollard hybrid approach

testcontainers-rs `ExecCommand` does NOT support stdin. To pipe JSON input to validators, we combine:
1. **testcontainers-rs** for container lifecycle (start, cleanup, port mapping)
2. **bollard** for exec with stdin (via `docker_client_instance()` + `create_exec` with `attach_stdin: true`)

```rust
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use testcontainers::core::client::docker_client_instance;
use bollard::exec::{CreateExecOptions, StartExecOptions};
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;

// 1. Start container with testcontainers-rs (handles lifecycle + file copy)
let container = GenericImage::new("osquery/osquery", "5.12.1-ubuntu22.04")
    .with_copy_to("/validate.sh", include_bytes!("../validators/validate-osquery.sh").to_vec())
    .start()
    .await?;

// 2. Get container ID and bollard client
let container_id = container.id();
let docker = docker_client_instance().await?;

// 3. Create exec WITH stdin attached (bollard API)
let exec_id = docker.create_exec(
    container_id,
    CreateExecOptions {
        attach_stdin: Some(true),   // ← Enable stdin!
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        cmd: Some(vec!["sh", "/validate.sh"]),
        ..Default::default()
    }
).await?.id;

// 4. Start exec and pipe JSON input
let bollard::exec::StartExecResults::Attached { mut input, mut output } =
    docker.start_exec(&exec_id, Some(StartExecOptions::default())).await?
else { return Err(anyhow!("Exec not attached")); };

// 5. Write JSON to stdin and close
input.write_all(input_json.as_bytes()).await?;
drop(input);  // Close stdin to signal EOF

// 6. Collect output and check exit code
// ... (see full example in starter code section)

// Container cleanup happens automatically when `container` is dropped
```

**Why this works**: testcontainers-rs uses bollard internally. `docker_client_instance()` returns `Result<Docker, ClientError>` (verified in [docs.rs](https://docs.rs/testcontainers/latest/testcontainers/core/client/fn.docker_client_instance.html)), and `container.id()` gives us the container ID to target with bollard's exec API.

**Success Criteria**: Can validate osquery SQL blocks, build fails on invalid SQL or schema errors

### Phase 1b: SQLite Validator
**Goal**: Add SQLite validation with setup blocks

**STATUS: COMPLETE** (Implemented 2025-11-27)

- [x] Add SQLite validator configuration (`keinos/sqlite3:3.47.2`)
- [x] Implement setup block handling via host-based validation:
  - Preprocessor runs SETUP SQL in container first
  - Preprocessor runs query with `-json` flag
  - Host validator script validates JSON output with `jq`
- [x] Validate output against assertions (`rows =`, `rows >=`, `rows >`, `contains`)
- [x] Validate output against expected JSON (`VALIDATOR_EXPECT`)
- [x] Strip all markers from output

**Implementation**: Uses host-based validation architecture:
1. Container: `sqlite3 /tmp/test.db "$SETUP_SQL"` (setup)
2. Container: `sqlite3 -json /tmp/test.db "$QUERY_SQL"` (query)
3. Host: `validators/validate-sqlite.sh` validates JSON with `jq`

**Test Coverage**: 15 tests in `tests/sqlite_validator_tests.rs`

**Success Criteria**: ✅ Can validate SQLite blocks with setup and assertions

### Phase 2: Transpiler (Marker Stripping)
**Goal**: Strip validation markers from rendered output

**STATUS: COMPLETE** (Implemented as part of Phase 1)

- [x] Implement `strip_markers()` function to remove <!--SETUP-->, <!--ASSERT-->, <!--EXPECT--> blocks
- [x] Implement `strip_double_at_lines()` for @@ hidden line removal
- [x] Integrate marker stripping in preprocessor after successful validation
- [x] Preserve code block structure while stripping markers only from validated blocks

**Implementation**: `src/transpiler.rs` (65 lines) with `strip_markers_from_chapter()` in `src/preprocessor.rs`

**Success Criteria**: Readers see clean code examples without validation artifacts

---

### Phase 3: Configuration
**Goal**: Parse validator configuration from book.toml

**STATUS: COMPLETE** (Implemented as part of Phase 1)

- [x] Define `ValidatorConfig` struct (container image, script path)
- [x] Define `Config` struct with validators HashMap and fail_fast flag
- [x] Parse [preprocessor.validator] section from book.toml
- [x] Support multiple validators with container caching

**Implementation**: `src/config.rs` (81 lines)

**Success Criteria**: Validators can be configured via book.toml

---

### Phase 4: Error Reporting
**Goal**: Make validation failures helpful

**STATUS: COMPLETE** (Implemented as part of Phase 1)

- [x] Show chapter name in error messages
- [x] Show validator name (if configured)
- [x] Display exit code
- [x] Display visible content (what user wrote)
- [x] Display validator stderr output
- [x] Display validator stdout output
- [x] Support "skip" annotation for intentionally broken examples

**Implementation**: Error formatting in `src/preprocessor.rs` lines 224-242, 296-310

**Remaining (not blocking v1):**
- [ ] Show approximate line number (via pulldown-cmark offset tracking)
- [ ] Add dry-run mode
- [ ] Validate marker syntax (error on unclosed markers)

**Success Criteria**: When validation fails, users know what's wrong and how to fix it

---

### Phase 5: osquery Config Validation
**Goal**: Add JSON config validation for osquery

**STATUS: NOT STARTED**

**IMPORTANT**: osquery configs are JSON, not TOML! From osquery docs:
> "By default, osqueryd will look for a JSON file on disk... The filesystem plugin architecture expects config plugins to yield valid JSON."

- [ ] Create JSON config validator script (validators/validate-osquery-config.sh)
- [ ] Test with osquery config files using `osqueryd --config_check`
- [ ] Add pyproject.toml validator (this one IS TOML, validated by validate-pyproject)

**Success Criteria**: Can validate osquery JSON configs and Python pyproject.toml files

### Phase 6: Shell Script Validators
**Goal**: Add ShellCheck and bash execution validators

**STATUS: NOT STARTED**

- [ ] ShellCheck validator using `koalaman/shellcheck-alpine:v0.10.0` (NOT the scratch-based image)
- [ ] Bash execution validator with post-execution assertions
- [ ] Support assertions: exit_code, file_exists, stdout_contains, etc.

**Container note**: The base `koalaman/shellcheck` image is scratch-based with NO shell. Must use `shellcheck-alpine` variant which includes ash/bash.

**Success Criteria**: Can validate shell scripts with both static analysis and execution

---

### Phase 7: Performance & Reliability
**Goal**: Make builds reasonably fast

**STATUS: PARTIALLY COMPLETE**

**Realistic expectations**:
- Container startup: 10-20 seconds per validator type
- testcontainers-rs Issue #742 (container reuse) is still open
- Target: 50 validations in < 3 minutes

- [x] Keep container handles alive for entire build via struct field (HashMap caching in preprocessor.rs)
- [ ] Add benchmark suite to track performance
- [x] Using testcontainers + bollard hybrid for container management
- [ ] Consider "external container mode" where user pre-starts containers

**Success Criteria**: Builds complete in reasonable time for books with <100 validated blocks

## Validator Script Packaging

Validator scripts must be available inside containers. The plan supports three strategies:

**ACTUAL IMPLEMENTATION:** Using Strategy 2 (Runtime File Copy) - validator scripts are loaded from disk at runtime via the `script` config field in book.toml. This allows users to customize validators without rebuilding the binary.

### Strategy 1: Embedded Scripts (NOT USED)

Validator scripts are compiled into the mdbook-validator binary using `include_bytes!`:

```rust
// src/validators.rs
pub const OSQUERY_VALIDATOR: &[u8] = include_bytes!("../validators/validate-osquery.sh");
pub const SQLITE_VALIDATOR: &[u8] = include_bytes!("../validators/validate-sqlite.sh");
pub const SHELLCHECK_VALIDATOR: &[u8] = include_bytes!("../validators/validate-shellcheck.sh");

// Usage: copy into container at startup
container = GenericImage::new("osquery/osquery", "5.12.1-ubuntu22.04")
    .with_copy_to("/validate.sh", OSQUERY_VALIDATOR.to_vec())
    .start()
    .await?;
```

**Pros**: Single binary distribution, no external files needed, works in CI/CD
**Cons**: Requires rebuild to update validators

### Strategy 2: Runtime File Copy (CURRENTLY USED)

Load validator scripts from disk at runtime:

```rust
let validator_path = config.validator_path.as_ref()
    .unwrap_or(&PathBuf::from("validators/validate-osquery.sh"));
let validator_bytes = std::fs::read(validator_path)?;

container = GenericImage::new(...)
    .with_copy_to("/validate.sh", validator_bytes)
    .start()
    .await?;
```

**Pros**: Edit validators without rebuilding, good for development
**Cons**: Requires validators directory alongside book

### Strategy 3: Custom Container Images (Production)

Bake validators into custom Docker images:

```dockerfile
# Dockerfile.osquery-validator
FROM osquery/osquery:5.17.0-ubuntu22.04
RUN apt-get update && apt-get install -y jq && rm -rf /var/lib/apt/lists/*
COPY validators/validate-osquery.sh /validate.sh
RUN chmod +x /validate.sh
```

```toml
# book.toml - reference custom image
[preprocessor.validator.validators.osquery]
container = "myregistry/osquery-validator:1.0"
validate-command = "/validate.sh"
```

**Pros**: Fastest startup (no copy step), includes dependencies (jq)
**Cons**: Must maintain Dockerfiles, rebuild images for validator changes

### Validator Dependencies

Each validator requires `jq` for JSON parsing. Dependency availability by base image:

| Base Image | Has bash | Has jq | Notes |
|------------|----------|--------|-------|
| osquery/osquery:5.17.0-ubuntu22.04 | ✅ | ❌ Install needed | Use Strategy 3 or install at runtime |
| keinos/sqlite3:3.47.2 | ✅ | ❌ | Alpine-based, use `apk add jq` |
| koalaman/shellcheck-alpine:stable | ✅ ash | ❌ | Use `apk add jq` |
| python:3.12-slim-bookworm | ✅ | ❌ | Use `apt-get install jq` |
| ubuntu:22.04 | ✅ | ❌ | Use `apt-get install jq` |

**Recommendation for v1**: Use Strategy 1 (embedded) with custom images (Strategy 3) for validators needing jq. This provides:
- Simple distribution (single binary)
- Reliable dependencies (baked into images)
- Works in CI/CD without file paths

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

Validator scripts receive input via **environment variables** and return **structured JSON** to stdout.

### Why This Design?

**Input (env vars)**: Simple, no `jq` dependency, bollard has native support.

**Output (JSON)**: Assertions parsed in Rust avoid bash regex fragility with quotes/special chars.

### Validator Output Protocol

Validators output a single JSON object to stdout:

```json
{
  "success": true,
  "output": "[{\"id\": 1}, {\"id\": 2}]",
  "row_count": 2,
  "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `success` | bool | Did the command execute successfully? |
| `output` | string | Raw output from the command (for EXPECT matching) |
| `row_count` | number \| null | Number of rows returned (for SQL validators) |
| `error` | string \| null | Error message if execution failed |

**Exit codes**:
- Exit 0 = execution succeeded (check `success` field for result)
- Exit non-zero = validator script itself failed (bug in validator)

### Assertion Parsing (Rust Side)

Assertions are parsed and evaluated in Rust, not bash. This handles:
- Strings with embedded quotes: `contains "key: \"value\""`
- Special regex characters: `matches "user\.name"`
- Numeric comparisons with proper typing

```rust
// src/assertions.rs
use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ValidatorOutput {
    pub success: bool,
    pub output: String,
    pub row_count: Option<i64>,
    pub error: Option<String>,
}

pub enum Assertion {
    RowsEqual(i64),
    RowsGreaterEqual(i64),
    RowsGreater(i64),
    Contains(String),
    Matches(String),  // Regex pattern
    ExitCode(i32),
    // File assertions handled separately in bash-exec
}

impl Assertion {
    pub fn parse(line: &str) -> Result<Self> {
        let line = line.trim();

        if let Some(rest) = line.strip_prefix("rows = ") {
            return Ok(Assertion::RowsEqual(rest.trim().parse()?));
        }
        if let Some(rest) = line.strip_prefix("rows >= ") {
            return Ok(Assertion::RowsGreaterEqual(rest.trim().parse()?));
        }
        if let Some(rest) = line.strip_prefix("rows > ") {
            return Ok(Assertion::RowsGreater(rest.trim().parse()?));
        }
        if let Some(rest) = line.strip_prefix("contains ") {
            // Parse quoted string, handling escaped quotes
            let s = parse_quoted_string(rest)?;
            return Ok(Assertion::Contains(s));
        }
        if let Some(rest) = line.strip_prefix("matches ") {
            let s = parse_quoted_string(rest)?;
            return Ok(Assertion::Matches(s));
        }
        if let Some(rest) = line.strip_prefix("exit_code = ") {
            return Ok(Assertion::ExitCode(rest.trim().parse()?));
        }

        Err(anyhow!("Unknown assertion: {}", line))
    }

    pub fn evaluate(&self, output: &ValidatorOutput) -> Result<()> {
        match self {
            Assertion::RowsEqual(expected) => {
                let actual = output.row_count.ok_or_else(||
                    anyhow!("Validator didn't return row_count"))?;
                if actual != *expected {
                    return Err(anyhow!(
                        "Assertion failed: rows = {}\n  Expected: {}\n  Actual: {}",
                        expected, expected, actual
                    ));
                }
            }
            Assertion::RowsGreaterEqual(min) => {
                let actual = output.row_count.ok_or_else(||
                    anyhow!("Validator didn't return row_count"))?;
                if actual < *min {
                    return Err(anyhow!(
                        "Assertion failed: rows >= {}\n  Actual: {}",
                        min, actual
                    ));
                }
            }
            Assertion::Contains(needle) => {
                if !output.output.contains(needle.as_str()) {
                    return Err(anyhow!(
                        "Assertion failed: contains \"{}\"\n  Output: {}",
                        needle, truncate(&output.output, 200)
                    ));
                }
            }
            Assertion::Matches(pattern) => {
                let re = regex::Regex::new(pattern)
                    .map_err(|e| anyhow!("Invalid regex '{}': {}", pattern, e))?;
                if !re.is_match(&output.output) {
                    return Err(anyhow!(
                        "Assertion failed: matches \"{}\"\n  Output: {}",
                        pattern, truncate(&output.output, 200)
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Parse a quoted string, handling escaped quotes
fn parse_quoted_string(s: &str) -> Result<String> {
    let s = s.trim();
    if !s.starts_with('"') || !s.ends_with('"') {
        return Err(anyhow!("String must be quoted: {}", s));
    }
    let inner = &s[1..s.len()-1];
    // Unescape \" to "
    Ok(inner.replace("\\\"", "\""))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}
```

### Why Not Parse Assertions in Bash?

The original bash-based assertion parsing broke on:

```bash
# This regex fails on embedded quotes:
if [[ "$line" =~ ^contains[[:space:]]+\"(.*)\"$ ]]; then
    SEARCH="${BASH_REMATCH[1]}"  # Broken if string contains \"
fi
```

Moving parsing to Rust provides:
- Proper string handling with escape sequences
- Type-safe numeric comparisons
- Regex compilation with useful error messages
- Consistent behavior across all validators

### Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `VALIDATOR_SETUP` | Setup content (SQL, bash, etc.) | `CREATE TABLE test (id INT);` |
| `VALIDATOR_CONTENT` | Main content to validate | `SELECT * FROM test;` |
| `VALIDATOR_ASSERTIONS` | Assertion rules (newline-separated) | `rows >= 1` |
| `VALIDATOR_EXPECT` | Expected output for exact matching | `[{"id": 1}]` |

### Content Size Limit

**Limit**: Code blocks are limited to ~30KB due to environment variable size constraints.

```rust
const MAX_CONTENT_SIZE: usize = 30_000; // 30KB limit

fn validate_content_size(content: &str) -> Result<()> {
    if content.len() > MAX_CONTENT_SIZE {
        return Err(anyhow!(
            "Code block exceeds maximum size of 30KB ({} bytes). \
             Consider splitting into smaller examples.",
            content.len()
        ));
    }
    Ok(())
}
```

**Rationale**: Documentation code examples should be concise. If a code block exceeds 30KB, it's likely too complex for documentation and should be split into smaller, focused examples.

### Rust Side (passing env vars)

```rust
let exec_id = docker.create_exec(
    container_id,
    CreateExecOptions {
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        env: Some(vec![
            format!("VALIDATOR_SETUP={}", setup.unwrap_or("")),
            format!("VALIDATOR_CONTENT={}", content),
            format!("VALIDATOR_ASSERTIONS={}", assertions.unwrap_or("")),
            format!("VALIDATOR_EXPECT={}", expect.unwrap_or("")),
        ]),
        cmd: Some(vec!["sh", "/validate.sh"]),
        ..Default::default()
    },
).await?.id;
```

### Bash Side (reading env vars)

```bash
#!/bin/bash
# No jq needed! Variables are already set.
echo "Setup: $VALIDATOR_SETUP"
echo "Content: $VALIDATOR_CONTENT"
echo "Assertions: $VALIDATOR_ASSERTIONS"
```

### Validator: osquery SQL

```bash
#!/bin/bash
# validate-osquery.sh
# Input: VALIDATOR_CONTENT (env var)
# Output: JSON with success, output, row_count, error

# Execute query - note: must use stdin pipe for osqueryi
OUTPUT=$(echo "$VALIDATOR_CONTENT" | osqueryi --json 2>&1)
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    # Output JSON error response (escape quotes in error message)
    ERROR_MSG=$(echo "$OUTPUT" | sed 's/"/\\"/g' | tr '\n' ' ')
    echo "{\"success\": false, \"output\": \"\", \"row_count\": null, \"error\": \"$ERROR_MSG\"}"
    exit 0  # Exit 0 because the validator ran successfully; error is in JSON
fi

# Count rows (count opening braces that start JSON objects)
ROW_COUNT=$(echo "$OUTPUT" | grep -c '^{' || echo 0)

# Escape the output for JSON (handle quotes and newlines)
ESCAPED_OUTPUT=$(echo "$OUTPUT" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | tr '\n' ' ')

# Output JSON success response
echo "{\"success\": true, \"output\": \"$ESCAPED_OUTPUT\", \"row_count\": $ROW_COUNT, \"error\": null}"
exit 0
```

**Note**: Validators always exit 0 and report errors in the JSON `error` field. Exit non-zero only for validator bugs (script errors), not validation failures.

### Validator: osquery Config (JSON)

```bash
#!/bin/bash
# validate-osquery-config.sh
# Input: VALIDATOR_CONTENT (env var)
set -e

TMPFILE=$(mktemp --suffix=.conf)
echo "$VALIDATOR_CONTENT" > "$TMPFILE"
trap "rm -f $TMPFILE" EXIT

# Validate with osquery config checker
osqueryd --config_path="$TMPFILE" --config_check --verbose 2>&1
exit $?
```

### Validator: SQLite (with setup)

```bash
#!/bin/bash
# validate-sqlite.sh
# Input: VALIDATOR_SETUP, VALIDATOR_CONTENT (env vars)
# Output: JSON with success, output, row_count, error

# Helper to output JSON error
json_error() {
    local msg=$(echo "$1" | sed 's/"/\\"/g' | tr '\n' ' ')
    echo "{\"success\": false, \"output\": \"\", \"row_count\": null, \"error\": \"$msg\"}"
    exit 0
}

DB_FILE=$(mktemp)  # Portable: no --suffix on BSD/macOS
DB_FILE="${DB_FILE}.db"
trap "rm -f $DB_FILE" EXIT

# Run setup SQL separately (solves multiple-SELECT JSON issue)
if [ -n "$VALIDATOR_SETUP" ]; then
    SETUP_ERROR=$(echo "$VALIDATOR_SETUP" | sqlite3 "$DB_FILE" 2>&1)
    if [ $? -ne 0 ]; then
        json_error "Setup SQL failed: $SETUP_ERROR"
    fi
fi

# Run query and capture JSON output
OUTPUT=$(echo "$VALIDATOR_CONTENT" | sqlite3 -json "$DB_FILE" 2>&1)
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    json_error "Query execution failed: $OUTPUT"
fi

# Count rows (sqlite3 -json outputs array, count objects)
ROW_COUNT=$(echo "$OUTPUT" | grep -c '^{' || echo 0)

# Escape the output for JSON
ESCAPED_OUTPUT=$(echo "$OUTPUT" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | tr '\n' ' ')

# Output JSON success response
echo "{\"success\": true, \"output\": \"$ESCAPED_OUTPUT\", \"row_count\": $ROW_COUNT, \"error\": null}"
exit 0
```

**Note**: EXPECT matching is now handled in Rust by comparing `output` field against expected value. Assertions are also evaluated in Rust using the structured response.

### Validator: pyproject.toml

```bash
#!/bin/bash
# validate-pyproject.sh
# Input: VALIDATOR_CONTENT (env var)
set -e

TMPFILE=$(mktemp --suffix=.toml)
echo "$VALIDATOR_CONTENT" > "$TMPFILE"
trap "rm -f $TMPFILE" EXIT

# validate-pyproject should be pre-installed in container image
validate-pyproject "$TMPFILE"
```

**Container requirement**: Build custom image with validate-pyproject pre-installed:
```dockerfile
FROM python:3.12-slim-bookworm
RUN pip install --no-cache-dir 'validate-pyproject[all]'
# Note: jq no longer needed!
```

### Validator: ShellCheck

```bash
#!/bin/sh
# validate-shellcheck.sh
# Input: VALIDATOR_CONTENT (env var)
# Note: Uses /bin/sh for Alpine compatibility

TMPFILE=$(mktemp)
echo "$VALIDATOR_CONTENT" > "$TMPFILE"
trap "rm -f $TMPFILE" EXIT

shellcheck -s bash "$TMPFILE"
```

**Container requirement**: Must use `koalaman/shellcheck-alpine:v0.10.0`, NOT the scratch-based image.

### Validator: Bash Execution

```bash
#!/bin/bash
# validate-bash-exec.sh
# Input: VALIDATOR_SETUP, VALIDATOR_CONTENT, VALIDATOR_ASSERTIONS (env vars)
set -e

# Run setup if provided
if [ -n "$VALIDATOR_SETUP" ]; then
    eval "$VALIDATOR_SETUP"
fi

TMPFILE=$(mktemp --suffix=.sh)
echo "$VALIDATOR_CONTENT" > "$TMPFILE"
chmod +x "$TMPFILE"

STDOUT=$(mktemp)
STDERR=$(mktemp)
trap "rm -f $TMPFILE $STDOUT $STDERR" EXIT

set +e
bash "$TMPFILE" > "$STDOUT" 2> "$STDERR"
ACTUAL_EXIT_CODE=$?
set -e

# Check assertions
if [ -n "$VALIDATOR_ASSERTIONS" ]; then
    # exit_code = N
    if echo "$VALIDATOR_ASSERTIONS" | grep -qE "^exit_code = [0-9]+"; then
        EXPECTED=$(echo "$VALIDATOR_ASSERTIONS" | grep -E "^exit_code = " | awk '{print $3}')
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
    done <<< "$VALIDATOR_ASSERTIONS"

    # dir_exists /path
    while IFS= read -r line; do
        if [[ "$line" =~ ^dir_exists[[:space:]]+(.+)$ ]]; then
            DIRPATH="${BASH_REMATCH[1]}"
            if [ ! -d "$DIRPATH" ]; then
                echo "Assertion failed: dir_exists $DIRPATH" >&2
                exit 1
            fi
        fi
    done <<< "$VALIDATOR_ASSERTIONS"

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
    done <<< "$VALIDATOR_ASSERTIONS"

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
    done <<< "$VALIDATOR_ASSERTIONS"
fi

# Default: require exit code 0 if no exit_code assertion
if ! echo "$VALIDATOR_ASSERTIONS" | grep -q "^exit_code"; then
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
default-timeout-secs = 30  # Default timeout for all validators

# Validators - use specific tags, NOT :latest
[preprocessor.validator.validators.osquery]
container = "osquery/osquery:5.17.0-ubuntu22.04"
validate-command = "/validators/validate-osquery.sh"
timeout-secs = 60  # Override: osquery startup is slow

[preprocessor.validator.validators.osquery-config]
container = "osquery/osquery:5.17.0-ubuntu22.04"
validate-command = "/validators/validate-osquery-config.sh"

[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
validate-command = "/validators/validate-sqlite.sh"

[preprocessor.validator.validators.pyproject]
container = "mdbook-validator/python-validate:3.12"  # Custom image with validate-pyproject
validate-command = "/validators/validate-pyproject.sh"

[preprocessor.validator.validators.shellcheck]
container = "koalaman/shellcheck-alpine:v0.10.0"  # Alpine variant has shell, pinned version
validate-command = "/validators/validate-shellcheck.sh"

[preprocessor.validator.validators.bash-exec]
container = "ubuntu:22.04"
validate-command = "/validators/validate-bash-exec.sh"
timeout-secs = 120  # Override: script execution may be slow
```

### Configuration Options

| Option | Scope | Default | Description |
|--------|-------|---------|-------------|
| `fail-fast` | Global | `true` | Stop on first validation failure |
| `default-timeout-secs` | Global | `30` | Default timeout for validators |
| `container` | Per-validator | Required | Docker image with specific tag |
| `validate-command` | Per-validator | Required | Path to validator script in container |
| `timeout-secs` | Per-validator | Inherits global | Override timeout for this validator |

## Error Handling

### Example Error Messages

**Schema drift (osquery updated)**:
```
Error: Validation failed in src/network-queries.md

  | ```sql validator=osquery
  | SELECT local_port, remote_address FROM listening_ports;
  | ```

Validator stderr:
  Error: no such table: listening_ports
```

**Assertion failure**:
```
Error: Assertion failed in src/examples.md

  | ```sql validator=sqlite
  | SELECT COUNT(*) as total FROM users
  | ```

Assertion failed: rows = 5
  Expected: 5
  Actual:   2

Validator output:
  [{"total": 2}]
```

**osquery config error**:
```
Error: Validation failed in src/config.md

  | ```json validator=osquery-config
  | {
  |   "options": {
  |     "logger_path": "/var/log/osquery"
  |   }
  | }

Validator stderr:
  Error reading config: parse error at line 3
```

**Note**: Error messages show the validator's raw output. We don't attempt to diagnose or interpret errors—the validator knows best what went wrong.

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
| Container execution | testcontainers-rs + bollard hybrid | testcontainers for lifecycle, bollard for exec with env vars |
| Validator input | Environment variables only | Simple; bollard API support; ~30KB practical limit |
| Validator output | Exit code + stderr | ~~Structured JSON~~ Simpler; exit 0=pass, non-zero=fail |
| Assertion parsing | Script-side evaluation | ~~Rust-side~~ Allows user customization of validators |
| Validator delivery | Runtime file loading | ~~Embedded via include_bytes!~~ More flexible for users |
| Execution timeout | ~~Configurable per-validator~~ | Not yet implemented |
| Docker availability | Explicit startup check | Clear error message if Docker not running |
| Block markers | SETUP/ASSERT/EXPECT only | SETUP is validator-interpreted (SQL for sqlite, bash for others) |
| Hidden lines | `@@` prefix | Language-agnostic; show partial configs while validating complete ones |
| osquery config format | JSON | osquery requires JSON, not TOML |
| Container tags | Specific versions | `:latest` and `:stable` are unpredictable |
| ShellCheck container | `shellcheck-alpine:v0.10.0` | Scratch image has no shell; pin version, not `:stable` |
| SQLite multiple SELECT | Run SETUP separately | sqlite3 -json produces invalid JSON with multiple SELECTs |
| Performance target | ~3 min for 50 blocks | Container startup is 10-20s each |
| Reusable setups | Not in v1 | Keep it simple; inline everything |
| Async runtime | tokio with `block_on` bridge | Required for bollard; see Async/Sync Bridging section |
| mdbook version | 0.4.x via `mdbook` crate | ~~0.5.1 via mdbook_preprocessor~~ Using standard mdbook crate |
| Markdown preservation | pulldown-cmark reconstruction | ~~Byte-offset surgery~~ Simpler event-based approach |

**Note**: ~~strikethrough~~ indicates original plan that was changed during implementation.

## Known Limitations

1. **30KB content size limit**: Code blocks cannot exceed ~30KB due to environment variable size constraints. This is intentional - documentation examples should be concise. Split large examples into smaller, focused blocks.

2. **No container reuse**: testcontainers-rs Issue #742 is still open. Each build starts fresh containers.

3. **Windows stdin pipe bug**: osqueryi has a known bug (#7972) where stdin piping fails on Windows with "incomplete SQL" errors. Related issue #6787 shows PowerShell-specific failures.

4. **Marker collision**: If your SQL contains `-->`, it will break marker parsing. Future: support configurable markers per-block.

5. **No line numbers**: Error messages show file but not exact line numbers (would require offset-to-line mapping).

## Markdown Preservation via Byte-Offset Surgery

We use pulldown-cmark's `into_offset_iter()` to get the exact byte ranges of code blocks, then splice the transformed content directly into the original source. This preserves all original formatting.

### Implementation

```rust
use pulldown_cmark::{Parser, Event, Tag, CodeBlockKind, Options};
use std::ops::Range;

/// A code block with its source location
struct LocatedCodeBlock {
    info_string: String,
    content_range: Range<usize>,  // Byte range of content within the code fence
    full_range: Range<usize>,     // Byte range of entire fenced block including ```
}

/// Find all code blocks with their source byte ranges
fn find_code_blocks(source: &str) -> Vec<LocatedCodeBlock> {
    let parser = Parser::new_ext(source, Options::all());
    let mut blocks = Vec::new();
    let mut current_info = String::new();
    let mut content_start = 0;

    for (event, range) in parser.into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                current_info = info.to_string();
                content_start = range.end; // Content starts after opening ```
            }
            Event::End(Tag::CodeBlock(CodeBlockKind::Fenced(_))) => {
                // The range here covers the entire block including closing ```
                // But we need to find where content ends (before closing ```)
                let content_end = source[..range.end]
                    .rfind("\n```")
                    .unwrap_or(range.end);

                blocks.push(LocatedCodeBlock {
                    info_string: std::mem::take(&mut current_info),
                    content_range: content_start..content_end,
                    full_range: range,
                });
            }
            _ => {}
        }
    }
    blocks
}

/// Transform source by replacing code block contents
fn transform_source(source: &str, replacements: &[(Range<usize>, String)]) -> String {
    let mut result = String::with_capacity(source.len());
    let mut last_end = 0;

    // Sort replacements by start position
    let mut sorted: Vec<_> = replacements.iter().collect();
    sorted.sort_by_key(|(range, _)| range.start);

    for (range, new_content) in sorted {
        // Copy unchanged content before this block
        result.push_str(&source[last_end..range.start]);
        // Insert transformed content
        result.push_str(new_content);
        last_end = range.end;
    }

    // Copy remaining content after last replacement
    result.push_str(&source[last_end..]);
    result
}
```

### What Gets Replaced

For a validated code block like:
````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE test (id INT);
-->
SELECT * FROM test;
<!--ASSERT
rows >= 1
-->
```
````

We:
1. Parse to find the block's content range (everything between opening and closing ```)
2. Validate the full content including markers
3. Generate clean content: `SELECT * FROM test;\n`
4. Replace ONLY the content bytes, preserving the fence markers and info string

Result: The original ` ```sql validator=sqlite ` line stays exactly as written.

## Async/Sync Bridging

mdBook preprocessors implement a synchronous trait, but bollard requires an async runtime. Here's how to bridge them:

### The Problem

```rust
// mdBook's Preprocessor trait is synchronous (mdbook_preprocessor 0.5.x)
impl Preprocessor for ValidatorPreprocessor {
    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        // But bollard operations are async!
        // self.runner.execute(...).await  // ← Can't await in sync function!
    }
}
```

### Solution: Create Runtime Per Build

```rust
use tokio::runtime::Builder;

pub struct ValidatorPreprocessor {
    // Don't store the runtime - create fresh for each build
}

impl Preprocessor for ValidatorPreprocessor {
    fn name(&self) -> &str {
        "validator"
    }

    fn run(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book, Error> {
        // Use current-thread runtime to avoid nested runtime panics
        let rt = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("Failed to create async runtime: {}", e)))?;

        // Run all async validation inside the runtime
        rt.block_on(async {
            self.run_async(ctx, book).await
        })
    }
}

impl ValidatorPreprocessor {
    async fn run_async(&self, ctx: &PreprocessorContext, book: Book) -> Result<Book> {
        let runner = ValidatorRunner::new().await?;
        let mut pool = ContainerPool::new();

        // Now we can use async/await normally
        for chapter in book.iter() {
            for block in find_code_blocks(chapter) {
                let container = pool.get_or_create(...).await?;
                runner.execute(container, ...).await?;
            }
        }

        Ok(book)
    }
}
```

### Why This Works

1. **Fresh runtime per build**: Each `mdbook build` invocation creates a new runtime. This avoids issues with runtime persistence across mdbook's watch mode.

2. **`block_on` bridges sync to async**: The synchronous `run()` method blocks until all async validation completes.

3. **Container pool lives within runtime**: The `ContainerPool` and its containers are created and dropped within the same runtime scope.

4. **bollard requires async**: We use bollard's `create_exec` with environment variables, which is async-only.

## Success Metrics

For v1 release, we've succeeded if:

1. ✅ osquery SQL queries validate against real osquery (catches schema drift)
2. ❌ osquery JSON configs validate with config checker (Phase 5 - not started)
3. ❌ pyproject.toml validates against PEP standards (Phase 5 - not started)
4. ❌ Shell scripts pass ShellCheck analysis (Phase 6 - not started)
5. ❌ Shell scripts run and pass execution assertions (Phase 6 - not started)
6. ✅ SQLite queries work with setup and assertions (Phase 1b - complete)
7. ✅ Clear error messages show what failed and why
8. ✅ Zero false positives (84 tests passing)
9. ✅ Build fails when docs don't match tool behavior
10. ❌ At least one external project adopts it (pending)

## Resources

- [mdBook preprocessor docs](https://rust-lang.github.io/mdBook/for_developers/preprocessors.html)
- [mdbook_preprocessor crate (0.5.x)](https://docs.rs/mdbook_preprocessor/latest/mdbook_preprocessor/)
- [testcontainers-rs docs](https://docs.rs/testcontainers/latest/testcontainers/)
- [testcontainers-rs docker_client_instance](https://docs.rs/testcontainers/latest/testcontainers/core/client/fn.docker_client_instance.html)
- [bollard Docker API docs](https://docs.rs/bollard/latest/bollard/)
- [bollard CreateExecOptions](https://docs.rs/bollard/latest/bollard/exec/struct.CreateExecOptions.html)
- [bollard attach_container example](https://github.com/fussybeaver/bollard/blob/master/examples/attach_container.rs)
- [pulldown-cmark docs](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/)
- [pulldown-cmark into_offset_iter](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Parser.html#method.into_offset_iter)
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
cargo add mdbook_preprocessor anyhow serde serde_json tracing tracing-subscriber
cargo add pulldown-cmark  # Use into_offset_iter() for byte spans
cargo add testcontainers  # async only, no blocking feature
cargo add bollard
cargo add tokio --features=rt
cargo add futures-util
cargo add regex  # For assertion pattern matching

# Create initial structure
mkdir -p src tests/fixtures validators

# Start implementing src/main.rs
```

**Note**: We use async testcontainers exclusively. The `blocking` feature is NOT used because we need bollard's async exec API. See "Async/Sync Bridging" section for how to integrate with mdBook's sync Preprocessor trait.

**Note**: We do NOT use `pulldown-cmark-to-cmark`. Instead, we use byte-offset surgery to preserve original markdown formatting.

## Prototype Test (Run This First!)

Before implementing the full system, validate the core data flow works:

```rust
// tests/prototype_test.rs
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
use testcontainers::core::client::docker_client_instance;
use bollard::exec::{CreateExecOptions, StartExecOptions};
use futures_util::StreamExt;

#[tokio::test]
async fn test_env_var_data_flow() {
    // Minimal validator that echoes environment variables
    let validator = br#"#!/bin/sh
echo "Setup: $VALIDATOR_SETUP"
echo "Content: $VALIDATOR_CONTENT"
exit 0
"#;

    // 1. Start container with validator script
    let container = GenericImage::new("alpine", "3")
        .with_copy_to("/validate.sh", validator.to_vec())
        .start()
        .await
        .expect("Container should start");

    // 2. Get container ID and docker client
    let container_id = container.id();
    let docker = docker_client_instance().await.expect("Docker client");

    // 3. Create exec with environment variables (no stdin needed!)
    let exec_id = docker.create_exec(
        container_id,
        CreateExecOptions {
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            env: Some(vec![
                "VALIDATOR_SETUP=CREATE TABLE test;".to_string(),
                "VALIDATOR_CONTENT=SELECT 1;".to_string(),
                "VALIDATOR_ASSERTIONS=".to_string(),
                "VALIDATOR_EXPECT=".to_string(),
            ]),
            cmd: Some(vec!["sh", "/validate.sh"]),
            ..Default::default()
        }
    ).await.expect("Create exec").id;

    // 4. Start exec and collect output
    let bollard::exec::StartExecResults::Attached { output, .. } =
        docker.start_exec(&exec_id, Some(StartExecOptions::default())).await.expect("Start exec")
    else { panic!("Exec should be attached"); };

    // 5. Collect output
    let mut stdout = Vec::new();
    let mut output = output;
    while let Some(Ok(log)) = output.next().await {
        if let bollard::container::LogOutput::StdOut { message } = log {
            stdout.extend(message);
        }
    }

    // 6. Verify
    let output_str = String::from_utf8_lossy(&stdout);
    assert!(output_str.contains("SELECT 1"), "Should see content: {}", output_str);
    assert!(output_str.contains("CREATE TABLE"), "Should see setup: {}", output_str);

    println!("✅ Prototype test passed! Environment variable data flow works.");
}
```

Run with: `cargo test test_env_var_data_flow -- --nocapture`

If this passes, the core architecture is viable. If it fails, investigate before building the full system.

## Initial Implementation Starter

### src/main.rs
```rust
use anyhow::Result;
use mdbook_preprocessor::{Preprocessor, parse_input};
use std::io;

fn main() -> Result<()> {
    let preprocessor = mdbook_validator::ValidatorPreprocessor::new()?;

    // mdBook 0.5.x: parse_input is now a standalone function
    let (ctx, book) = parse_input(io::stdin())?;

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
pub mod assertions;
pub mod container;
pub mod config;

pub use preprocessor::ValidatorPreprocessor;
pub use assertions::{Assertion, ValidatorOutput};
```

### src/validator.rs (core execution logic)
```rust
use anyhow::{anyhow, Result};
use bollard::exec::{CreateExecOptions, StartExecOptions, StartExecResults};
use bollard::Docker;
use futures_util::StreamExt;
use std::time::Duration;
use testcontainers::core::client::docker_client_instance;
use testcontainers::core::ContainerAsync;
use testcontainers::GenericImage;
use tokio::time::timeout;

/// Maximum content size (env var limit)
const MAX_CONTENT_SIZE: usize = 30_000;

/// Input to a validator script
pub struct ValidatorInput<'a> {
    pub setup: Option<&'a str>,
    pub content: &'a str,
    pub assertions: Option<&'a str>,
    pub expect: Option<&'a str>,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub success: bool,
    pub exit_code: i64,
    pub stdout: String,
    pub stderr: String,
}

pub struct ValidatorRunner {
    docker: Docker,
}

impl ValidatorRunner {
    /// Create a new runner, checking Docker availability
    pub async fn new() -> Result<Self> {
        let docker = docker_client_instance().await
            .map_err(|e| anyhow!(
                "Failed to connect to Docker. Is Docker running?\n\
                 Error: {}\n\n\
                 To skip validation, add 'skip' to code block annotations.",
                e
            ))?;

        // Verify Docker is responsive
        docker.ping().await
            .map_err(|e| anyhow!(
                "Docker daemon not responding. Is Docker running?\n\
                 Error: {}",
                e
            ))?;

        Ok(Self { docker })
    }

    /// Execute a validator script inside a running container
    pub async fn execute(
        &self,
        container: &ContainerAsync<GenericImage>,
        validator_cmd: &str,
        input: &ValidatorInput<'_>,
        timeout_secs: u64,
    ) -> Result<ValidationResult> {
        // Check content size limit
        if input.content.len() > MAX_CONTENT_SIZE {
            return Err(anyhow!(
                "Code block exceeds maximum size of 30KB ({} bytes). \
                 Consider splitting into smaller examples.",
                input.content.len()
            ));
        }

        let result = timeout(
            Duration::from_secs(timeout_secs),
            self.execute_inner(container, validator_cmd, input)
        ).await;

        match result {
            Ok(inner_result) => inner_result,
            Err(_) => Err(anyhow!(
                "Validation timed out after {} seconds. \
                 Consider increasing timeout_secs in book.toml or simplifying the example.",
                timeout_secs
            )),
        }
    }

    async fn execute_inner(
        &self,
        container: &ContainerAsync<GenericImage>,
        validator_cmd: &str,
        input: &ValidatorInput<'_>,
    ) -> Result<ValidationResult> {
        let container_id = container.id();

        // Build environment variables
        let env_vars = vec![
            format!("VALIDATOR_SETUP={}", input.setup.unwrap_or("")),
            format!("VALIDATOR_CONTENT={}", input.content),
            format!("VALIDATOR_ASSERTIONS={}", input.assertions.unwrap_or("")),
            format!("VALIDATOR_EXPECT={}", input.expect.unwrap_or("")),
        ];

        // Create exec with environment variables
        let exec_id = self
            .docker
            .create_exec(
                container_id,
                CreateExecOptions {
                    attach_stdout: Some(true),
                    attach_stderr: Some(true),
                    tty: Some(false),
                    env: Some(env_vars),
                    cmd: Some(vec!["sh", "/validate.sh"]),
                    ..Default::default()
                },
            )
            .await?
            .id;

        // Start exec and collect output
        let start_result = self
            .docker
            .start_exec(&exec_id, Some(StartExecOptions::default()))
            .await?;

        let StartExecResults::Attached { output, .. } = start_result else {
            return Err(anyhow!("Exec command did not attach"));
        };

        // Collect stdout and stderr
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut output = output;

        while let Some(result) = output.next().await {
            match result? {
                bollard::container::LogOutput::StdOut { message } => {
                    stdout.extend_from_slice(&message);
                }
                bollard::container::LogOutput::StdErr { message } => {
                    stderr.extend_from_slice(&message);
                }
                _ => {}
            }
        }

        // Get exit code
        let inspect = self.docker.inspect_exec(&exec_id).await?;
        let exit_code = inspect.exit_code.unwrap_or(-1);

        Ok(ValidationResult {
            success: exit_code == 0,
            exit_code,
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
        })
    }
}
```

**Key features**:
- Docker availability check with clear error messages
- 30KB content size limit with clear error
- Configurable timeout per validation

### src/container.rs (container lifecycle)
```rust
use anyhow::Result;
use std::collections::HashMap;
use testcontainers::core::ContainerAsync;
use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

/// Embedded validator scripts (compiled into binary)
pub mod embedded {
    pub const OSQUERY: &[u8] = include_bytes!("../validators/validate-osquery.sh");
    pub const SQLITE: &[u8] = include_bytes!("../validators/validate-sqlite.sh");
    // Add more as needed
}

pub struct ContainerPool {
    containers: HashMap<String, ContainerAsync<GenericImage>>,
}

impl ContainerPool {
    pub fn new() -> Self {
        Self {
            containers: HashMap::new(),
        }
    }

    /// Get or create a container for the given validator type
    pub async fn get_or_create(
        &mut self,
        validator_name: &str,
        image: &str,
        tag: &str,
        validator_script: &[u8],
    ) -> Result<&ContainerAsync<GenericImage>> {
        if !self.containers.contains_key(validator_name) {
            let container = GenericImage::new(image, tag)
                .with_copy_to("/validate.sh", validator_script.to_vec())
                .start()
                .await?;

            self.containers.insert(validator_name.to_string(), container);
        }

        Ok(self.containers.get(validator_name).unwrap())
    }
}
```

### src/parser.rs (with bug fixes)
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

/// Process @@ hidden lines: returns (visible_content, validation_content)
/// Preserves trailing newlines.
fn process_hidden_lines(content: &str) -> (String, String) {
    let mut visible_lines = Vec::new();
    let mut all_lines = Vec::new();
    let ends_with_newline = content.ends_with('\n');

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

    let mut visible = visible_lines.join("\n");
    let mut all = all_lines.join("\n");

    // Preserve trailing newline if original had one
    if ends_with_newline {
        visible.push('\n');
        all.push('\n');
    }

    (visible, all)
}

/// Extract content between marker_start and marker_end.
/// Handles markers at start/end of content and preserves whitespace structure.
fn extract_marker(
    content: &str,
    marker_start: &str,
    marker_end: &str,
) -> Result<(String, Option<String>)> {
    let Some(start_idx) = content.find(marker_start) else {
        return Ok((content.to_string(), None));
    };

    let after_marker_start = start_idx + marker_start.len();
    let search_area = &content[after_marker_start..];

    let end_idx = search_area
        .find(marker_end)
        .ok_or_else(|| anyhow!(
            "Unclosed marker: found '{}' without matching '{}'",
            marker_start,
            marker_end
        ))?;

    // Extract the marker content, trimming leading/trailing whitespace
    let marker_content = search_area[..end_idx].trim().to_string();

    // Build remaining content:
    // - Everything before the marker start
    // - Everything after the marker end
    let before = &content[..start_idx];
    let after = &search_area[end_idx + marker_end.len()..];

    // Join with appropriate whitespace handling
    let remaining = if before.is_empty() {
        after.trim_start().to_string()
    } else if after.is_empty() {
        before.trim_end().to_string()
    } else {
        // Both have content - join with single newline if there was whitespace
        let before_trimmed = before.trim_end();
        let after_trimmed = after.trim_start();
        if before_trimmed.is_empty() {
            after_trimmed.to_string()
        } else if after_trimmed.is_empty() {
            before_trimmed.to_string()
        } else {
            format!("{}\n{}", before_trimmed, after_trimmed)
        }
    };

    Ok((remaining, Some(marker_content)))
}

impl CodeBlock {
    pub fn parse(
        info_string: &str,
        content: &str,
        markers: &MarkerConfig,
    ) -> Result<Self> {
        let (language, validator, skip) = parse_info_string(info_string);

        // Extract block markers in order: SETUP, then ASSERT, then EXPECT
        // Each extraction removes that marker from the content
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

        // Process @@ hidden lines on the remaining content
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hidden_lines_preserves_trailing_newline() {
        let input = "line1\n@@hidden\nline2\n";
        let (visible, all) = process_hidden_lines(input);
        assert!(visible.ends_with('\n'), "Should preserve trailing newline");
        assert_eq!(visible, "line1\nline2\n");
        assert_eq!(all, "line1\nhidden\nline2\n");
    }

    #[test]
    fn test_hidden_lines_no_trailing_newline() {
        let input = "line1\n@@hidden\nline2";
        let (visible, all) = process_hidden_lines(input);
        assert!(!visible.ends_with('\n'), "Should not add trailing newline");
        assert_eq!(visible, "line1\nline2");
    }

    #[test]
    fn test_extract_marker_at_start() {
        let content = "<!--SETUP\nCREATE TABLE test;\n-->\nSELECT * FROM test;";
        let markers = MarkerConfig::default();
        let (remaining, setup) = extract_marker(
            content,
            &markers.setup_start,
            &markers.setup_end,
        ).unwrap();

        assert_eq!(setup, Some("CREATE TABLE test;".to_string()));
        assert_eq!(remaining, "SELECT * FROM test;");
    }

    #[test]
    fn test_extract_marker_at_end() {
        let content = "SELECT * FROM test;\n<!--ASSERT\nrows >= 1\n-->";
        let markers = MarkerConfig::default();
        let (remaining, assertions) = extract_marker(
            content,
            &markers.assert_start,
            &markers.assert_end,
        ).unwrap();

        assert_eq!(assertions, Some("rows >= 1".to_string()));
        assert_eq!(remaining, "SELECT * FROM test;");
    }

    #[test]
    fn test_full_parse() {
        let content = r#"<!--SETUP
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1);
-->
SELECT * FROM test;
<!--ASSERT
rows = 1
-->"#;

        let block = CodeBlock::parse("sql validator=sqlite", content, &MarkerConfig::default()).unwrap();

        assert_eq!(block.language, "sql");
        assert_eq!(block.validator, Some("sqlite".to_string()));
        assert!(block.setup.is_some());
        assert!(block.setup.unwrap().contains("CREATE TABLE"));
        assert_eq!(block.visible_content.trim(), "SELECT * FROM test;");
        assert!(block.assertions.is_some());
    }
}
```

---

## Summary

Key design decisions:

1. **Hybrid container approach** - testcontainers-rs for lifecycle + bollard for exec (API verified)
2. **Environment variables only** - Simple input; 30KB limit enforced (docs examples should be concise)
3. **Structured JSON output** - Validators return JSON; assertions parsed in Rust (not bash)
4. **Rust-side assertion evaluation** - Handles embedded quotes, special chars, proper typing
5. **Embedded validator scripts** - `include_bytes!` for single-binary distribution
6. **Configurable execution timeout** - Default 30s; prevents hung builds
7. **Docker availability check** - Clear error message if Docker not running
8. **Three block markers**: SETUP, ASSERT, EXPECT - SETUP is validator-interpreted
9. **`@@` line prefix** - Hide context lines while validating complete content
10. **osquery configs are JSON** - NOT TOML (osquery requires JSON)
11. **Specific container tags** - No `:latest` or `:stable` (e.g., `osquery/osquery:5.17.0-ubuntu22.04`)
12. **ShellCheck container** - Must use Alpine variant with pinned version (scratch image has no shell)
13. **SQLite** - Run SETUP separately from query to avoid invalid JSON
14. **Async/sync bridging** - Use `Builder::new_current_thread()` to avoid nested runtime panics
15. **Realistic performance** - 10-20s container startup, ~3 min for 50 blocks
16. **Inline setup only (v1)** - No reusable setup blocks from book.toml
17. **mdBook 0.5.1** - Use `mdbook_preprocessor` crate; `parse_input` is standalone function
18. **Byte-offset surgery** - Use `into_offset_iter()` for source spans; splice original markdown
19. **Prototype first** - Run the prototype test to validate core data flow before full implementation
