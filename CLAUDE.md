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
   CREATE TABLE alerts (path TEXT, scanner TEXT);
   INSERT INTO alerts VALUES ('/data/test.json', 'scanner1');
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
   - Spins up specified container via testcontainers-rs
   - Runs validator script via `sh -c "..."` wrapper with structured JSON: `{setup, content, assertions, expect}`
   - Validator interprets SETUP appropriately (SQL for sqlite, bash for others)
   - Fails build if execution fails OR assertions don't match
   - Strips all markers and returns only visible content to mdBook

3. Result: Clean examples in published docs, all guaranteed to work AND produce expected output

**Reader sees**: `SELECT path FROM alerts WHERE path LIKE '%.json'`
**Validator tested**: CREATE + INSERT + SELECT + output assertions

## Tech Stack

- **Rust 2021** - preprocessor is a Rust binary
- **mdbook** - preprocessor interface
- **testcontainers-rs** - manages Docker containers (blocking feature)
- **pulldown-cmark** - parses markdown
- **pulldown-cmark-to-cmark** - reconstructs markdown after modification
- **Containers** (specific tags, NOT :latest):
  - `osquery/osquery:5.12.1-ubuntu22.04` - osquery SQL and JSON config validation
  - `keinos/sqlite3:3.47.2` - SQLite validation with setup support
  - `koalaman/shellcheck-alpine:stable` - shell script static analysis

## Project Status

**Current**: Planning/early implementation
**Phase**: Phase 1 - Core preprocessor MVP
**Next milestone**: Validate one SQL block against osquery

## Key Design Decisions

1. **External validator scripts** - Flexibility over embedding validators in binary
2. **Container-first** - Reproducible, isolated validation environments
3. **Opt-in validation** - Only validate blocks with `validator=` attribute
4. **Fail-fast** - Stop build immediately on first error (configurable)
5. **Structured input** - Validators receive JSON: `{setup, content, assertions, expect}`
6. **Transpilation approach** - Validate with setup/assertions, render without them
7. **Three marker system** - `<!--SETUP-->` + `<!--ASSERT-->` + `<!--EXPECT-->`
8. **SETUP is validator-interpreted** - SQLite treats as SQL; bash-exec treats as shell commands
9. **Inline setup only (v1)** - No reusable setup blocks from book.toml
10. **Shell wrapper required** - `sh -c "..."` for all container exec (testcontainers limitation)
11. **Specific container tags** - Never use `:latest`
12. **osquery config is JSON** - NOT TOML (osquery requires JSON)

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

```
Markdown → Parser → Extract blocks with validator= attribute →
Extract block markers: <!--SETUP-->, <!--ASSERT-->, <!--EXPECT--> →
Process @@ lines: strip prefix, mark for removal from output →
Start container →
  1. Run SETUP (validator interprets appropriately)
  2. Run content (visible lines + @@ lines with prefix stripped)
  3. Validate output against ASSERT/EXPECT →
Exit 0? → Strip markers and @@ lines → Return clean content to mdBook
Exit non-0? → Fail build with error
```

## File Organization

```
src/
  main.rs          - CLI entry point, mdBook integration
  lib.rs           - Public API
  preprocessor.rs  - Main preprocessor logic (implements Preprocessor trait)
  parser.rs        - Markdown parsing, extract code blocks + markers
  transpiler.rs    - Strip markers from validated blocks for final output
  validator.rs     - Orchestrates validation
  container.rs     - Container lifecycle (start, exec, cleanup)
  config.rs        - Parse book.toml configuration

tests/
  integration_tests.rs - Full preprocessor tests
  fixtures/            - Test books and validators

validators/
  validate-osquery.sh        - osquery SQL validator
  validate-osquery-config.sh - osquery JSON config validator
  validate-sqlite.sh         - SQLite validator with setup support
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

Inline setup (hidden from docs):
````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE test (id INTEGER, name TEXT);
INSERT INTO test VALUES (1, 'alice');
-->
SELECT * FROM test WHERE id = 1;
```
````

Reader sees only: `SELECT * FROM test WHERE id = 1;`

With assertions (validate output):
````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1), (2), (3);
-->
SELECT COUNT(*) as total FROM test
<!--ASSERT
rows = 1
total = 3
-->
```
````

With expected output (exact match for regression testing):
````markdown
```sql validator=sqlite
<!--SETUP
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1), (2);
-->
SELECT id FROM test ORDER BY id
<!--EXPECT
[{"id": 1}, {"id": 2}]
-->
```
````

## Validator Script Contract

**Input**: Structured JSON via stdin
```json
{
  "setup": "CREATE TABLE test (id INTEGER);\nINSERT INTO test VALUES (1);",
  "content": "SELECT * FROM test;",
  "assertions": "rows >= 1",
  "expect": null
}
```

**Execution order in validator:**
1. Run `setup` content (validator interprets appropriately)
2. Execute `content` (the visible query)
3. Validate output against `assertions` and/or `expect`

**Output**: Exit 0 = pass, non-zero = fail
**Error reporting**: Write to stderr

**SQLite validator approach** (solves multiple-SELECT JSON issue):
```bash
# Run setup SQL separately (no JSON output needed)
if [ -n "$SETUP" ]; then
    echo "$SETUP" | sqlite3 "$DB_FILE"
fi
# Run query and capture JSON output
OUTPUT=$(echo "$CONTENT" | sqlite3 -json "$DB_FILE")
```

## Configuration (book.toml)

```toml
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
```

## Current Tasks (Phase 1 MVP)

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

## Testing Strategy

**Unit tests**: Each module has unit tests for its logic
**Integration tests**: Full preprocessor runs against `tests/fixtures/test-book/`
**Manual testing**: Run against real osquery docs

Test fixtures include:
- `valid-examples.md` - Should pass validation
- `invalid-examples.md` - Should fail validation

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

### Container Execution
**Critical**: testcontainers-rs `exec()` doesn't support shell piping. All validator invocations must use:
```rust
container.exec(ExecCommand::new(["sh", "-c", "cat /tmp/input.json | /validators/validate-osquery.sh"]))
```

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

## Dependencies (Cargo.toml)

```toml
[dependencies]
mdbook = "0.4"
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
pulldown-cmark = "0.13"
pulldown-cmark-to-cmark = "21"
testcontainers = { version = "0.23", features = ["blocking"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Quick Start for Development

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

# Run tests
cargo test

# Try it with example book
cd examples/osquery-docs
mdbook build
```

## Known Limitations

1. **No container reuse**: testcontainers-rs Issue #742 is still open. Each build starts fresh containers.

2. **Windows stdin pipe bug**: osqueryi has a known bug (#7972) where stdin piping fails on Windows with "incomplete SQL" errors.

3. **Marker collision**: If your SQL contains `-->`, it will break marker parsing. Use unique markers in config if needed.

4. **No line numbers**: Error messages show file but not exact line numbers (would require offset-to-line mapping).

5. **No reusable setups (v1)**: All setup content must be inline. No book.toml setup blocks.

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
4. Spins up Docker containers via testcontainers (blocking feature)
5. Runs validator scripts via `sh -c "..."` with structured JSON: `{setup, content, assertions, expect}`
6. Validators run code AND validate output (assertions + exact matching)
7. Strips all markers and `@@` lines from output on success, fails build on validation/assertion error

**Key insight**: Acts as validator + transpiler + regression tester. Validates execution AND output, renders clean examples.

**Critical details**:
- osquery config is JSON, not TOML
- Use specific container tags (e.g., `osquery/osquery:5.12.1-ubuntu22.04`), never `:latest`
- testcontainers exec needs `sh -c "..."` wrapper
- SQLite: run SETUP separately from query to avoid invalid JSON output
- `@@` prefix hides context lines (validate complete config, show only relevant portion)

Target: osquery SQL/JSON config validation. Make documentation examples impossible to break AND guarantee correct output.
