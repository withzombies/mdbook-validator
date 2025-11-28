# CLAUDE.md - Project Context

## What is this?

`mdbook-validator` is an mdBook preprocessor that validates code blocks during documentation builds using Docker containers, while acting as a transpiler to hide validation-only context from final documentation. It validates both execution AND output. Think CI/CD + regression testing for your documentation examples, with setup code that readers never see.

## Why does it exist?

Documentation code examples rot. SQL queries reference non-existent tables, configs have typos, examples break when the tool updates. Worse, examples might execute but produce unexpected output.

**The problem**: To validate a SQL query, you need setup code (CREATE TABLE, INSERT test data). To ensure it works correctly, you need to check the output. But readers don't need to see setup code or validation logic.

**The solution**: Write examples with hidden setup blocks and output assertions that are validated but stripped from output. Clean docs, validated examples, guaranteed correct output.

**Primary use case**: Validating osquery SQL queries and JSON configs against a live osquery container.

## How it works

1. User writes markdown with annotated code blocks, optional hidden setup, and optional output validation:
   ````markdown
   ```sql validator=sqlite
   <!--SETUP
   sqlite3 /tmp/test.db << 'EOF'
   CREATE TABLE alerts (path TEXT, scanner TEXT);
   INSERT INTO alerts VALUES ('/data/test.json', 'scanner1');
   EOF
   -->
   SELECT path FROM alerts WHERE path LIKE '%.json'
   <!--ASSERT
   rows >= 1
   contains "test.json"
   -->
   ```
   ````

2. During `mdbook build`, the preprocessor:
   - Parses markdown looking for blocks with `validator=` attribute
   - Extracts visible content, setup blocks, assertions, and expected output
   - Spins up specified container via testcontainers-rs (container provides tool only)
   - Runs setup SQL in container (if any)
   - Runs query in container → captures JSON output
   - Runs validator script on HOST with JSON stdin (jq available for parsing)
   - Validator checks assertions against JSON output
   - Fails build if execution fails OR assertions don't match
   - Strips all markers and returns only visible content to mdBook

3. Result: Clean examples in published docs, all guaranteed to work AND produce expected output

**Reader sees**: `SELECT path FROM alerts WHERE path LIKE '%.json'`
**Validator tested**: CREATE + INSERT + SELECT + output assertions

## Tech Stack

- **Rust 2021** - preprocessor is a Rust binary
- **mdbook** - preprocessor interface
- **testcontainers-rs** - manages Docker containers (async, with bollard for exec)
- **pulldown-cmark** - parses markdown
- **pulldown-cmark-to-cmark** - reconstructs markdown after modification
- **Containers** (specific tags, NOT :latest):
  - `osquery/osquery:5.17.0-ubuntu22.04` - osquery SQL and JSON config validation
  - `keinos/sqlite3:3.47.2` - SQLite validation with setup support
  - `koalaman/shellcheck-alpine:stable` - shell script static analysis

## Project Status

**Current**: Project scaffolding in progress
**Phase**: Phase 0 - Project Setup (Epic: `mdbook-validator-wkm`)
**Completed**:
- Cargo.toml with dependencies and strict clippy lints
- rustfmt.toml with Edition 2021 style

**In Progress**:
- deny.toml for dependency license scanning

**Remaining** (Phase 0):
- `.config/nextest.toml` test profiles
- `.cargo-husky/hooks/` pre-commit and pre-push hooks
- Source module stubs (error.rs, preprocessor.rs, parser.rs, etc.)
- Test structure (tests/integration_tests.rs, fixtures)

**Next Phase**: Phase 1 - Core preprocessor MVP

## Key Design Decisions

1. **Host-based validation** - Validators run on HOST (not in containers), enabling jq for JSON parsing
2. **Container-first for tools** - Containers provide reproducible tool environments (sqlite3, osqueryi)
3. **Opt-in validation** - Only validate blocks with `validator=` attribute
4. **Fail-fast** - Stop build immediately on first error (configurable)
5. **JSON-based data flow** - Query runs in container with -json flag, output piped to host validator
6. **Transpilation approach** - Validate with setup/assertions, render without them
7. **Three marker system** - `<!--SETUP-->` + `<!--ASSERT-->` + `<!--EXPECT-->`
8. **SETUP runs in container** - Setup SQL executes before query to prepare test data
9. **Inline setup only (v1)** - No reusable setup blocks from book.toml
10. **Host validators use jq** - Simpler than installing jq in each container image
11. **Specific container tags** - Never use `:latest`
12. **osquery config is JSON** - NOT TOML (osquery requires JSON)

## Marker System (Simplified)

### Block Markers (stripped from output)

| Marker | Purpose | How It Works |
|--------|---------|--------------|
| `<!--SETUP-->` | Shell command(s) to run before query | Content IS the shell command - runs via `sh -c` |
| `<!--ASSERT-->` | Output validation rules | Row counts, contains, patterns |
| `<!--EXPECT-->` | Exact output matching | JSON comparison for regression tests |

**Design principle**: SETUP content IS the shell command - the preprocessor runs it directly via `sh -c "$SETUP_CONTENT"`. This unified approach works for any validator (sqlite3, osqueryi, etc.).

### Line Prefix: `@@` (hidden context lines)

Lines starting with `@@` are:
1. Sent to the validator (with `@@` stripped)
2. Removed from rendered output

**Use case**: Show only relevant portions of a config while validating the complete file.

````markdown
```toml validator=dlp-config
@@watch_paths = ["/home/%%"]
@@exclude_paths = []
@@
[policies]
enabled_policies = ["ccpa"]

[policies.policy_configs.ccpa]
enabled = true
@@
@@[work_queue]
@@max_queue_size = 10000
```
````

**Reader sees** only the `[policies]` section.
**Validator receives** the complete config.

**Key benefit**: Language-agnostic. Works with TOML, JSON, YAML, SQL, or any format.

## Architecture Quick Reference

**Host-based validation**: Containers run tools (sqlite3, osqueryi), validators run on HOST with jq.

```
Markdown → Parser → Extract blocks with validator= attribute →
Extract block markers: <!--SETUP-->, <!--ASSERT-->, <!--EXPECT--> →
Process @@ lines: strip prefix, mark for removal from output →
Start container (tool only, no scripts injected) →
  1. Run SETUP in container (e.g., sqlite3 /tmp/db "CREATE TABLE...")
  2. Run query in container → get JSON output (e.g., sqlite3 -json /tmp/db "SELECT...")
  3. Run validator on HOST with JSON stdin (jq available!)
     - Validator checks assertions against JSON output
     - Validator checks EXPECT for exact match
Exit 0? → Strip markers and @@ lines → Return clean content to mdBook
Exit non-0? → Fail build with error
```

**Data flow:**
```
SETUP SQL → container (sqlite3) → (no output needed)
QUERY SQL → container (sqlite3 -json) → JSON stdout
                ↓
         HOST validator.sh (stdin = JSON)
                ↓
         jq parses JSON, checks rows/columns/contains
                ↓
         exit 0 = pass, exit non-0 = fail
```

## File Organization

```
src/
  main.rs           - CLI entry point, mdBook integration
  lib.rs            - Public API
  preprocessor.rs   - Main preprocessor logic (implements Preprocessor trait)
  parser.rs         - Markdown parsing, extract code blocks + markers
  transpiler.rs     - Strip markers from validated blocks for final output
  container.rs      - Container lifecycle (start_raw, exec_raw for tool execution)
  host_validator.rs - Host-side validator execution (spawns validator scripts locally with JSON stdin)
  config.rs         - Parse book.toml configuration

tests/
  integration_tests.rs - Full preprocessor tests
  fixtures/            - Test books and validators

validators/
  validate-osquery.sh        - osquery SQL validator (runs on HOST, uses jq)
  validate-osquery-config.sh - osquery JSON config validator (runs on HOST)
  validate-sqlite.sh         - SQLite validator (runs on HOST, uses jq for assertions)
```

## Code Block Annotation Syntax

Info string format: `language validator=name [skip]`

### osquery Examples

Basic query (validates syntax and schema against real osquery):
````markdown
```sql validator=osquery
SELECT uid, username, shell FROM users LIMIT 5;
```
````

With assertions:
````markdown
```sql validator=osquery
SELECT uid, username, shell FROM users WHERE username = 'root'
<!--ASSERT
rows >= 1
contains "root"
-->
```
````

### osquery Config (JSON, NOT TOML!)

**IMPORTANT**: osquery configs are JSON, not TOML!

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

### SQLite Examples (with setup)

Inline setup (hidden from docs) - SETUP content IS the shell command:
````markdown
```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db 'CREATE TABLE test (id INTEGER, name TEXT); INSERT INTO test VALUES (1, "alice");'
-->
SELECT * FROM test WHERE id = 1;
```
````

Reader sees only: `SELECT * FROM test WHERE id = 1;`

Multi-line setup with heredoc (for complex schemas):
````markdown
```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db << 'EOF'
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1), (2), (3);
EOF
-->
SELECT COUNT(*) as total FROM test
<!--ASSERT
rows = 1
total = 3
-->
```
````

With fixtures directory (mount local files to /fixtures):
````markdown
```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db < /fixtures/schema.sql
-->
SELECT id FROM test ORDER BY id
<!--EXPECT
[{"id": 1}, {"id": 2}]
-->
```
````

## Validator Script Contract (Host-Based)

Validators run on HOST, not in containers. They receive query output and check assertions.

**Input:**
- **stdin**: JSON output from container query (e.g., `[{"id": 1}, {"id": 2}]`)
- **VALIDATOR_ASSERTIONS** env var: Assertion rules (e.g., `rows >= 1\ncontains "test"`)
- **VALIDATOR_EXPECT** env var: Expected output for exact matching (optional)

**Output:** Exit 0 = pass, non-zero = fail
**Error reporting:** Write to stderr

**Example validator script (validate-sqlite.sh):**
```bash
#!/bin/bash
# Reads JSON from stdin, checks assertions using jq

JSON_INPUT=$(cat)
ROW_COUNT=$(echo "$JSON_INPUT" | jq 'length')

# Check row assertions
if [[ "$VALIDATOR_ASSERTIONS" == *"rows >= "* ]]; then
    EXPECTED=$(echo "$VALIDATOR_ASSERTIONS" | grep -oP 'rows >= \K\d+')
    if [ "$ROW_COUNT" -lt "$EXPECTED" ]; then
        echo "Assertion failed: rows >= $EXPECTED (got $ROW_COUNT)" >&2
        exit 1
    fi
fi

# Check contains assertions
if [[ "$VALIDATOR_ASSERTIONS" == *"contains "* ]]; then
    NEEDLE=$(echo "$VALIDATOR_ASSERTIONS" | grep -oP 'contains "\K[^"]+')
    if ! echo "$JSON_INPUT" | jq -e --arg s "$NEEDLE" 'any(.. | strings; contains($s))' > /dev/null; then
        echo "Assertion failed: contains \"$NEEDLE\"" >&2
        exit 1
    fi
fi

exit 0
```

**Key design:** Validators don't run queries - they only validate JSON output from container. This separation keeps validators simple and portable.

## Configuration (book.toml)

```toml
[preprocessor.validator]
command = "mdbook-validator"
fail-fast = true

# Optional: Mount local fixtures directory to /fixtures in containers
# Useful for loading schema files, test data, etc.
# fixtures_dir = "fixtures"  # Relative to book root

# Validators - use specific tags, NOT :latest
# Note: Validators run on HOST, containers only provide tools
# Note: SETUP content IS the shell command - runs via sh -c

[preprocessor.validator.validators.osquery]
container = "osquery/osquery:5.17.0-ubuntu22.04"
script = "validators/validate-osquery.sh"  # Runs on HOST
# Optional: Override default query command
# query_command = "osqueryi --json"

[preprocessor.validator.validators.osquery-config]
container = "osquery/osquery:5.17.0-ubuntu22.04"
script = "validators/validate-osquery-config.sh"

[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
script = "validators/validate-sqlite.sh"  # Runs on HOST with jq
# Optional: Override default query command (default shown)
# query_command = "sqlite3 -json /tmp/test.db"
```

**Config fields:**
- `fail_fast` - Stop on first validation failure (default: true)
- `fixtures_dir` - Optional: Path to fixtures directory, mounted to /fixtures in containers
- `container` - Docker image for tool execution (sqlite3, osqueryi)
- `script` - Path to validator script (runs on HOST, receives JSON stdin)
- `query_command` - Optional: Command to run query in container (should output JSON)

## Current Tasks

Task tracking is managed via `bd` (beads). See `bd list` for current tasks.

**Current Epic**: `mdbook-validator-wkm` (Project Scaffolding)

## Testing Strategy

**Unit tests**: Each module has unit tests for its logic
**Integration tests**: Full preprocessor runs against `tests/fixtures/test-book/`
**Manual testing**: Run against real osquery docs

Test fixtures include:
- `valid-examples.md` - Should pass validation
- `invalid-examples.md` - Should fail validation

### TDD Principles (MANDATORY)

Follow RED-GREEN-REFACTOR:
1. **RED**: Write a failing test first
2. **GREEN**: Write minimal code to pass the test
3. **REFACTOR**: Clean up while keeping tests green

### Test Anti-Patterns (FORBIDDEN)

**NEVER skip tests based on environment:**
```rust
// ❌ WRONG - Hides real failures as "skipped"
if error_msg.contains("Docker") {
    println!("Skipping test - Docker not available");
    return;
}

// ✅ CORRECT - Test fails, problem is visible
panic!("Test failed: {e}");
```

**Why this matters:**
- Skip logic hides real bugs (e.g., wrong image tag returns 404, matches "docker", test "passes")
- Tests should fail loudly when requirements aren't met
- CI/CD should fail if Docker isn't available, not silently skip tests
- "Skipped" tests give false confidence

**NEVER use `#[ignore]` without a tracking issue:**
```rust
// ❌ WRONG - Test forgotten forever
#[ignore]
#[test]
fn broken_test() { ... }

// ✅ CORRECT - If must ignore, track it
#[ignore = "TODO(#123): Fix after upstream bug resolved"]
#[test]
fn temporarily_broken() { ... }
```

**NEVER catch-all error handling in tests:**
```rust
// ❌ WRONG - Masks which errors are expected
Err(e) => {
    if e.to_string().contains("expected") { return; }
    panic!("{e}");
}

// ✅ CORRECT - Be explicit about expected errors
Err(e) => {
    assert!(e.to_string().contains("Validation failed"));
}
```

### Test Requirements

- All tests MUST fail (panic) on unexpected errors
- All tests MUST pass in CI with Docker available
- Container tests use real containers, no mocking
- Coverage target: 80% on core modules (parser, transpiler, container, preprocessor)

## Important Implementation Notes

### Markdown Parsing
Use `pulldown-cmark` to parse markdown into events. Look for:
```rust
Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info_string)))
```

The `info_string` contains our annotation (e.g., "sql validator=sqlite").

Parse attributes from info string:
- `validator=name` - which validator to use
- `skip` - skip validation but keep in output

### Marker Extraction
Parse code block content looking for marker types:
1. Extract everything between `<!--SETUP` and `-->`
2. Extract everything between `<!--ASSERT` and `-->`
3. Extract everything between `<!--EXPECT` and `-->`
4. Store visible content (everything NOT in markers)
5. For validation: create JSON with `{setup, content, assertions, expect}`
6. For output: return only visible content

### Container Execution (Host-Based Architecture)

Containers only run tools (sqlite3, osqueryi). Validators run on HOST.

**Container API (exec_raw):**
```rust
// Run setup in container
container.exec_raw(&["sh", "-c", "sqlite3 /tmp/test.db \"CREATE TABLE test (id INT)\""]).await?;

// Run query in container, get JSON output
let result = container.exec_raw(&["sh", "-c", "sqlite3 -json /tmp/test.db \"SELECT * FROM test\""]).await?;
let json_output = result.stdout;  // JSON from query
```

**Host validation (host_validator::run_validator):**
```rust
// Validate JSON output on host (jq available!)
let validation_result = host_validator::run_validator(
    "validators/validate-sqlite.sh",  // script path
    &json_output,                      // JSON stdin
    Some("rows >= 1"),                 // assertions (env var)
    None,                              // expected output (env var)
)?;
```

**Why host-based?** Installing jq in each container is complex. Running validators on host means jq is always available.

### Transpilation Process
1. Parse markdown
2. Find blocks with `validator=` attribute
3. Extract and validate with full context + output checks
4. On success: emit markdown with ALL markers stripped
5. On failure: exit immediately with detailed error including output diff

### Error Reporting
On validation failure, show:
1. File (and line number if available)
2. Visible content (what user wrote)
3. Full validation content (with setup)
4. Which assertions failed (expected vs actual)
5. Output diff if EXPECT doesn't match
6. Validator stderr output

### Performance
- Container startup: expect 10-20 seconds per validator type (not 2-5s)
- testcontainers-rs Issue #742 (container reuse) is still open
- Target: 50 validations in < 3 minutes

## Common Pitfalls to Avoid

1. **Don't parse markdown twice** - Parse once, collect all blocks, validate in batch
2. **Don't leak containers** - Use testcontainers properly, it handles cleanup
3. **Don't swallow errors** - Propagate with context using `anyhow`
4. **Don't validate unannotated blocks** - Only validate blocks with `validator=` attribute
5. **Don't return setup in output** - Always strip markers before returning to mdBook
6. **Don't use :latest tags** - Use specific container versions
7. **Don't use raw exec()** - Wrap in `sh -c "..."` for shell features
8. **Don't put `-->` inside marker content** - The marker end sequence inside content will break parsing
9. **Don't combine SETUP and SELECT in sqlite3 -json** - Run SETUP separately (sqlite3 -json produces invalid JSON with multiple statements)
10. **osquery config is JSON, not TOML** - This is a common misconception
11. **Don't skip tests based on environment** - Tests must fail loudly, not silently skip (skip logic hides real bugs like wrong image tags)

## Dependencies (Cargo.toml)

```toml
[dependencies]
mdbook = "0.4"
anyhow = "1.0"
thiserror = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
pulldown-cmark = "0.13"
pulldown-cmark-to-cmark = "21"
testcontainers = { version = "0.23", features = ["blocking"] }
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
cargo-husky = { version = "1.5", features = ["precommit-hook", "run-cargo-clippy", "run-cargo-fmt"] }

[lints.clippy]
pedantic = { level = "deny", priority = -1 }
missing_errors_doc = "allow"
missing_panics_doc = "allow"
module_name_repetitions = "allow"
must_use_candidate = "allow"

[lints.rust]
unsafe_code = "deny"
```

## Development Setup

### Required Tools

```bash
# Install cargo-deny for license/vulnerability scanning
cargo install cargo-deny

# Install cargo-nextest for fast parallel testing
cargo install cargo-nextest
```

### Code Quality Commands

```bash
# Format check (runs on pre-commit)
cargo fmt --check

# Lint check with pedantic warnings as errors
cargo clippy --all-targets -- -D warnings

# License and vulnerability scanning
cargo deny check

# Run tests (parallel by default)
cargo nextest run

# Run tests including Docker integration tests (sequential)
cargo nextest run --profile docker-integration
```

### Lint Configuration

The project uses strict clippy lints (`pedantic = "deny"`) with these exceptions:
- `missing_errors_doc` - allowed (reduces documentation burden)
- `missing_panics_doc` - allowed (reduces documentation burden)
- `module_name_repetitions` - allowed (e.g., `ValidatorError` in `validator` module)
- `must_use_candidate` - allowed (reduces noise)

Unsafe code is forbidden (`unsafe_code = "deny"`).

## Quick Start for Development

```bash
# Clone and enter the project
git clone <repo-url>
cd mdbook-validator

# Build the project
cargo build

# Run code quality checks
cargo fmt --check
cargo clippy --all-targets -- -D warnings

# Run tests
cargo nextest run
```

## Known Limitations

1. **No container reuse**: testcontainers-rs Issue #742 is still open. Each build starts fresh containers.

2. **Windows stdin pipe bug**: osqueryi has a known bug (#7972) where stdin piping fails on Windows with "incomplete SQL" errors.

3. **Marker collision**: If your SQL contains `-->`, it will break marker parsing. Use unique markers in config if needed.

4. **No line numbers**: Error messages show file but not exact line numbers (would require offset-to-line mapping).

5. **No reusable setups (v1)**: All setup content must be inline. No book.toml setup blocks.

6. **rustfmt unstable features**: `imports_granularity` and `group_imports` in rustfmt.toml require nightly rustfmt. On stable, these produce warnings but don't affect formatting. The config is intentionally set for when stable rustfmt supports these features.

## Related Reading

- [mdBook preprocessor docs](https://rust-lang.github.io/mdBook/for_developers/preprocessors.html)
- [testcontainers-rs docs](https://docs.rs/testcontainers/latest/testcontainers/)
- [pulldown-cmark docs](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/)
- [pulldown-cmark-to-cmark docs](https://docs.rs/pulldown-cmark-to-cmark/latest/)
- [osquery schema reference](https://osquery.io/schema/)
- [osquery configuration (JSON)](https://osquery.readthedocs.io/en/stable/deployment/configuration/)

## Project Goals

**v1.0 Success Criteria**:
- Validates SQL against osquery (catches schema drift)
- Validates JSON configs against osquery
- Supports inline setup markers
- Supports `@@` hidden context lines (show partial, validate complete)
- Supports output assertions (rows, columns, contains, etc.)
- Supports expected output matching (regression testing)
- Strips all markers and `@@` lines from rendered output
- Clear error messages showing validation context AND output diffs
- Zero false positives
- Used by at least one external project

**Non-goals for v1**:
- Performance optimization (correctness first)
- Web UI (CLI only)
- Non-container validators
- Reusable setup blocks from book.toml
- Showing output in rendered book (that's mdbook-cmdrun's job)
- Nested markers
- Snapshot testing with auto-update

---

## TL;DR for Claude Code

Build an mdBook preprocessor in Rust that:
1. Parses markdown for blocks with `validator=` attribute (e.g., ` ```sql validator=osquery`)
2. Extracts block markers: `<!--SETUP-->`, `<!--ASSERT-->`, `<!--EXPECT-->`
3. Processes `@@` line prefix: hidden context lines sent to validator but stripped from output
4. Spins up Docker containers via testcontainers (container provides tool only)
5. Runs SETUP in container (e.g., `sqlite3 /tmp/db "CREATE TABLE..."`)
6. Runs query in container → captures JSON output (e.g., `sqlite3 -json /tmp/db "SELECT..."`)
7. Runs validator script on HOST with JSON stdin (jq available for parsing!)
8. Strips all markers and `@@` lines from output on success, fails build on validation/assertion error

**Key insight**: Host-based validation. Containers run tools, validators run on host with jq.

**Critical details**:
- osquery config is JSON, not TOML
- Use specific container tags (e.g., `osquery/osquery:5.17.0-ubuntu22.04`), never `:latest`
- Containers only run tools - no validator scripts copied into containers
- Validators run on HOST via `host_validator::run_validator()` with JSON stdin
- `@@` prefix hides context lines (validate complete config, show only relevant portion)

Target: osquery SQL/JSON config validation. Make documentation examples impossible to break AND guarantee correct output.
