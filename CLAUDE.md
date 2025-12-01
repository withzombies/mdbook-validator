# CLAUDE.md - mdbook-validator

## What Is This?

An mdBook preprocessor that validates code blocks against Docker containers during `mdbook build`. It strips validation markers from output, so readers see clean examples while CI catches documentation rot.

**Primary use case**: Validating osquery SQL queries and JSON configs against live containers.

## Why It Exists

Documentation examples break silently. SQL queries reference renamed tables, configs have typos, examples break when tools update. This preprocessor validates examples during build and fails if they don't work.

**Key insight**: Examples need setup code (CREATE TABLE, INSERT) and output assertions, but readers shouldn't see that. The preprocessor validates with full context, then strips it from output.

## Architecture (Host-Based Validation)

```
Markdown → Parser → Extract validator= blocks →
  → Run SETUP in container (e.g., sqlite3 CREATE TABLE)
  → Run query in container → JSON output
  → Run validator script on HOST with JSON stdin (jq available)
  → Strip markers → Clean output to mdBook
```

Containers provide tools (sqlite3, osqueryi). Validators run on host with jq for JSON parsing.

## File Structure

```
src/
  main.rs           - CLI entry, mdBook integration
  preprocessor.rs   - Main logic (Preprocessor trait)
  parser.rs         - Extract code blocks + markers
  transpiler.rs     - Strip markers for output
  container.rs      - Container lifecycle (testcontainers)
  host_validator.rs - Run validator scripts on host
  config.rs         - Parse book.toml
  error.rs          - Structured errors E001-E010

validators/
  validate-sqlite.sh        - SQLite (runs on HOST)
  validate-osquery.sh       - osquery SQL (runs on HOST)
  validate-osquery-config.sh - osquery JSON config
  validate-bash-exec.sh     - Bash execution
  validate-shellcheck.sh    - Shell static analysis
  validate-python.sh        - Python syntax
  validate-template.sh      - Template for new validators

tests/
  fixtures/test-book/       - E2E test mdbook
```

## Marker System

| Marker | Purpose |
|--------|---------|
| `<!--SETUP-->` | Shell commands run before query (hidden from output) |
| `<!--ASSERT-->` | Output validation: `rows >= 1`, `contains "text"` |
| `<!--EXPECT-->` | Exact JSON output matching |
| `@@` prefix | Hidden context lines (validate complete, show partial) |

See `validators/validate-template.sh` for validator contract details.

## Tech Stack

- **Rust 2021** with strict clippy (`pedantic = "deny"`)
- **testcontainers-rs** + bollard for Docker
- **pulldown-cmark** for markdown parsing
- Containers: `keinos/sqlite3:3.47.2`, `osquery/osquery:5.17.0-ubuntu22.04`, etc.

## How to Build & Test

```bash
# Build
cargo build

# Run tests (requires Docker)
cargo nextest run

# Quality checks
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Key Design Decisions

1. **Host-based validators** - Scripts run on host (jq available), not in containers
2. **Opt-in validation** - Only blocks with `validator=` attribute
3. **Fail-fast** - Stop on first error (configurable)
4. **Specific container tags** - Never use `:latest`
5. **SETUP is shell** - Content runs via `sh -c`, works for any tool
6. **osquery config is JSON** - Not TOML (common misconception)

## Error Codes

Structured errors E001-E010 defined in `src/error.rs`. See TROUBLESHOOTING.md for causes and fixes.

## Common Pitfalls

1. Don't use `:latest` tags - use specific versions
2. Don't put `-->` inside marker content - breaks parsing
3. Don't combine SETUP and SELECT in `sqlite3 -json` - run separately
4. osquery config is JSON, not TOML
5. Windows has osqueryi stdin bug (#7972) - use WSL

## Configuration

Validators configured in `book.toml`:

```toml
[preprocessor.validator]
command = "mdbook-validator"
fail-fast = true

[preprocessor.validator.validators.sqlite]
container = "keinos/sqlite3:3.47.2"
script = "validators/validate-sqlite.sh"
```

See existing validators in `validators/` for examples.

## Task Tracking

Uses `bd` (beads) for task management. Run `bd list` for current tasks.

## Quick Reference

```bash
# Run single test
cargo test test_name

# Verbose test output
cargo test -- --nocapture

# Check a specific validator
cat input.sql | validators/validate-sqlite.sh
```

## Related Docs

- TROUBLESHOOTING.md - Error codes E001-E010 with fixes
- CHANGELOG.md - Release history
- validators/validate-template.sh - Validator contract & examples
