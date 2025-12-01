# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2025-12-01

Initial release of mdbook-validator, an mdBook preprocessor that validates code examples against live Docker containers during documentation builds.

### Added

#### Validators
- **SQLite validator** (`validate-sqlite.sh`) - Validates SQL queries against SQLite with full SETUP block support for creating test data
- **osquery SQL validator** (`validate-osquery.sh`) - Validates SQL queries against a live osquery instance for schema verification
- **osquery config validator** (`validate-osquery-config.sh`) - Validates osquery JSON configuration files
- **Bash execution validator** (`validate-bash-exec.sh`) - Runs bash scripts and validates exit codes, stdout, and file system state
- **ShellCheck validator** (`validate-shellcheck.sh`) - Static analysis of shell scripts using ShellCheck
- **Python syntax validator** (`validate-python.sh`) - Validates Python syntax using the Python compiler
- **Validator template** (`validate-template.sh`) - Comprehensive template for creating custom validators

#### Block Markers
- `<!--SETUP-->` - Hidden setup code executed before visible content (e.g., CREATE TABLE statements)
- `<!--ASSERT-->` - Output validation rules (row counts, string matching, regex patterns)
- `<!--EXPECT-->` - Exact JSON output matching for regression testing

#### Hidden Context Lines
- `@@` line prefix - Lines sent to validator but stripped from rendered output
- Enables showing partial configs while validating complete files

#### SQL Assertions
- `rows = N` - Exact row count matching
- `rows >= N` - Minimum row count
- `contains "string"` - Output contains substring
- `matches "regex"` - Regex pattern matching

#### Bash Assertions
- `exit_code = N` - Validate script exit code (default: 0)
- `stdout_contains "string"` - Stdout content validation
- `file_exists /path` - File existence check
- `dir_exists /path` - Directory existence check
- `file_contains /path "string"` - File content validation

#### Configuration
- `book.toml` based configuration for validators
- Per-validator container image specification (with version tags)
- Per-validator script path configuration
- `fail-fast` option (default: true) to stop on first failure
- `fixtures_dir` option for mounting host directories into containers

#### Architecture
- Host-based validation - Validators run on host with `jq` for JSON parsing
- Containers provide tool environments only (sqlite3, osqueryi, etc.)
- Secure content passing via stdin (no shell injection)
- Full marker stripping - All validation markers removed from rendered output

#### Developer Experience
- Structured error types with stable codes (E001-E010)
- TROUBLESHOOTING.md with actionable fixes for all error codes
- Dependency detection at startup (jq, Docker)
- `skip` attribute to skip validation of intentionally broken examples

#### Testing
- 155 passing tests across 18 test files
- E2E integration tests with real mdbook builds
- Container image verification tests
- Strict Clippy lints with panic-free code requirements

### Dependencies

- mdBook 0.4
- testcontainers 0.23 for container management
- bollard 0.18 for Docker API
- pulldown-cmark 0.13 for markdown parsing
- Supports all mdBook renderers (HTML, EPUB, etc.)

### Container Images

Pre-configured for these container images (specific versions, not `:latest`):
- `keinos/sqlite3:3.47.2` - SQLite
- `osquery/osquery:5.17.0-ubuntu22.04` - osquery
- `koalaman/shellcheck-alpine:stable` - ShellCheck
- `ubuntu:22.04` - Bash execution
- `python:3.12-slim` - Python syntax checking

### Platform Support

- macOS (Docker Desktop)
- Linux (Docker daemon)
- Windows (Docker Desktop with WSL 2 recommended due to osqueryi stdin bug #7972)
