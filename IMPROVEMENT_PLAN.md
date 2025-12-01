# mdbook-validator Improvement Plan

**Created**: 2025-11-30
**Status**: Draft - Pending Prioritization
**Current State**: Production-ready, 155 tests passing, 95%+ coverage on core modules

---

## Executive Summary

This document outlines improvement opportunities for mdbook-validator across five categories:
1. Testing improvements
2. User experience improvements
3. Code quality improvements
4. Documentation improvements
5. Future feature ideas

Each section includes specific issues, recommended improvements, and effort estimates.

---

## 1. Testing Improvements

### 1.1 Identified Gaps

| ID | Area | Issue | Impact |
|----|------|-------|--------|
| T1 | Duplicate tests | `tests/transpiler_tests.rs` duplicates `src/transpiler.rs` unit tests | Maintenance burden, confusion |
| T2 | Legacy code | `tests/prototype_test.rs` (178 lines) may be obsolete | Clutter, maintenance cost |
| T3 | Validator input validation | Validators don't test malformed assertions like `rows = abc` | Could crash on bad user input |
| T4 | Uneven E2E coverage | shellcheck/python validators have less E2E coverage than sqlite/osquery | Lower confidence in those validators |
| T5 | No validator script unit tests | Shell scripts lack their own test suite | Validator bugs harder to catch |

### 1.2 Recommended Improvements

#### T1: Consolidate Transpiler Tests
**Effort**: 30 minutes
**Action**:
- Compare `tests/transpiler_tests.rs` with `src/transpiler.rs` tests
- Remove duplicates, keep only unique integration-level tests
- Or delete entirely if fully redundant

#### T2: Audit prototype_test.rs
**Effort**: 30 minutes
**Action**:
- Review what `tests/prototype_test.rs` tests
- If covered elsewhere, delete it
- If unique, document why it exists or migrate to proper test file

#### T3: Add Validator Script Input Validation Tests
**Effort**: 2 hours
**Action**: Create tests for each validator with invalid inputs:
- `rows = not_a_number` (non-integer)
- `rows >= -1` (edge case negative)
- Empty JSON input `{}`
- Malformed JSON input
- Unknown assertion syntax `foo = bar`
- Missing required fields

**Test locations**: New file `tests/validator_script_tests.rs` or inline in each validator test file

#### T4: Add shellcheck/python E2E Test Parity
**Effort**: 2 hours
**Action**: Match the coverage level of sqlite/osquery tests:
- Valid script passes
- Invalid script fails with correct error
- Assertions work (contains, etc.)
- SETUP block works (if applicable)
- Error messages are helpful

#### T5: Create Validator Script Test Suite
**Effort**: 4 hours
**Action**:
- Create `validators/test/` directory
- Add test harness that runs each validator with known inputs
- Test success cases, failure cases, edge cases
- Can be shell-based (`bats`) or integrated into Rust tests

---

## 2. User Experience Improvements

### 2.1 Setup Friction Points

| ID | Pain Point | Current State | User Impact |
|----|------------|---------------|-------------|
| U1 | jq dependency | Must install separately, no detection | Confusing errors if missing |
| U2 | Docker requirement | Silent failure if not running | "Works on my machine" issues |
| U3 | Validator scripts | Must manually copy to project | Error-prone setup |
| U4 | book.toml config | Verbose, easy to misconfigure | Frustrating onboarding |
| U5 | No validation-only mode | Must run full build to validate | Slow feedback loop |

### 2.2 Error Message Gaps

| ID | Issue | Current | Desired |
|----|-------|---------|---------|
| U6 | No line numbers | "chapter 'Setup Guide'" | "chapter 'Setup Guide' at line 47" |
| U7 | No hints | Raw error only | "Hint: Did your SETUP block create the data?" |
| U8 | No doc links | Error code only | "See: https://docs.example.com/errors#E003" |
| U9 | Poor JSON diffs | "Output mismatch" | Structured diff showing differences |

### 2.3 Recommended Improvements

#### U1: Detect jq at Startup
**Effort**: 1 hour
**Action**:
- Check for `jq` in PATH before running validators that need it
- Provide clear error: "jq is required for JSON validators. Install with: brew install jq (macOS) or apt-get install jq (Linux)"
- Could make jq optional if validator doesn't need JSON parsing

#### U2: Detect Docker at Startup
**Effort**: 1 hour
**Action**:
- Check Docker daemon is running before starting containers
- Provide clear error: "Docker is not running. Please start Docker Desktop or the Docker daemon."
- Include platform-specific instructions

#### U3: Add `mdbook-validator init` Command
**Effort**: 4 hours
**Action**:
- New CLI subcommand: `mdbook-validator init`
- Creates `validators/` directory with all validator scripts
- Optionally adds example config to book.toml
- Interactive mode to select which validators to install

```bash
$ mdbook-validator init
Creating validators directory...
  Created: validators/validate-sqlite.sh
  Created: validators/validate-osquery.sh
  Created: validators/validate-bash-exec.sh
  Created: validators/validate-shellcheck.sh
  Created: validators/validate-python.sh
  Created: validators/validate-template.sh

Add to your book.toml:
  [preprocessor.validator.validators.sqlite]
  container = "keinos/sqlite3:3.47.2"
  script = "validators/validate-sqlite.sh"
```

#### U4: Add `mdbook-validator check` Command
**Effort**: 3 hours
**Action**:
- New CLI subcommand: `mdbook-validator check`
- Validates book.toml configuration without running builds
- Checks:
  - All referenced validator scripts exist and are executable
  - Container images are valid format (not :latest)
  - Required fields present
  - No unknown fields (typo detection)

```bash
$ mdbook-validator check
Checking configuration...
  ✓ validators/validate-sqlite.sh exists and is executable
  ✓ validators/validate-osquery.sh exists and is executable
  ✗ validators/validate-foo.sh not found
  ✗ Container 'sqlite:latest' uses :latest tag (not recommended)

2 errors found. See TROUBLESHOOTING.md for help.
```

#### U5: Add Validation-Only Mode
**Effort**: 2 hours
**Action**:
- New CLI subcommand: `mdbook-validator validate`
- Runs validation without full mdbook build
- Faster feedback for authors
- Could target specific chapters: `mdbook-validator validate src/chapter1.md`

#### U6: Add Line Numbers to Errors
**Effort**: 4 hours
**Action**:
- Track source line numbers during markdown parsing
- Include in error messages: "at line 47"
- May require changes to parser.rs to track offsets

#### U7-U8: Add Error Hints and Documentation Links
**Effort**: 2 hours
**Action**:
- Extend error types with `hint()` method
- Add common hints for each error type:
  - E001 (Config): "Check book.toml syntax"
  - E003 (Validation): "Check SETUP block creates test data"
  - E005 (Container): "Is Docker running?"
- Add documentation URL to error output

#### U9: Improve JSON Diff Output
**Effort**: 3 hours
**Action**:
- When EXPECT doesn't match, show structured diff
- Highlight specific fields that differ
- Consider using `similar` crate for diff output

```
Output mismatch:
  Expected: [{"id": 1, "name": "alice"}]
  Actual:   [{"id": 1, "name": "bob"}]

  Diff:
    [0].name: expected "alice", got "bob"
```

---

## 3. Code Quality Improvements

### 3.1 Current Strengths
- Strict clippy (pedantic deny)
- No unsafe code
- Trait-based testing (DockerOperations, CommandRunner)
- Structured errors (E001-E010)
- 95%+ test coverage on core modules

### 3.2 Technical Debt

| ID | Area | Issue | Impact |
|----|------|-------|--------|
| C1 | Container reuse | Each test starts fresh containers (10-20s each) | Slow test suite |
| C2 | Memory usage | Full JSON loaded into memory | Could OOM on large outputs |
| C3 | Marker parsing | `-->` in content breaks parsing | Edge case failures |
| C4 | Windows support | osqueryi stdin bug (#7972) | Platform limitation |
| C5 | Async runtime | Some `block_on` usage could be cleaner | Code clarity |

### 3.3 Recommended Improvements

#### C1: Container Pooling (Blocked)
**Effort**: 8 hours (when unblocked)
**Status**: Blocked on testcontainers-rs Issue #742
**Action**:
- Monitor testcontainers-rs for container reuse feature
- When available, implement container pooling
- Expected 5-10x speedup for test suite

#### C2: Streaming Large Outputs
**Effort**: 6 hours
**Action**:
- Add streaming mode for JSON outputs > 10MB
- Process line-by-line instead of loading full output
- Low priority unless users hit this limit

#### C3: Robust Marker Parsing
**Effort**: 4 hours
**Action**:
- Option A: Use unique delimiters (`<!--MDBOOK-VALIDATOR-SETUP-->`)
- Option B: Support escape sequences (`--\>` inside content)
- Option C: Document limitation clearly
- Recommend Option C for now (document limitation)

#### C4: Windows Documentation
**Effort**: 1 hour
**Action**:
- Document osqueryi stdin pipe bug (#7972) in README
- Provide workaround if available
- Add to TROUBLESHOOTING.md

#### C5: Async Cleanup
**Effort**: 4 hours
**Action**:
- Audit `block_on` usage
- Convert to proper async where beneficial
- Low priority (works correctly, just not idiomatic)

---

## 4. Documentation Improvements

### 4.1 Current State
- CLAUDE.md: Comprehensive developer documentation
- No user-facing README
- No troubleshooting guide
- No quick-start guide

### 4.2 Recommended Documents

| ID | Document | Purpose | Priority | Effort |
|----|----------|---------|----------|--------|
| D1 | README.md | Quick start for end users | High | 2 hours |
| D2 | TROUBLESHOOTING.md | Common errors and fixes | High | 2 hours |
| D3 | VALIDATORS.md | How to create custom validators | Medium | 3 hours |
| D4 | PERFORMANCE.md | Tips for faster builds | Low | 1 hour |
| D5 | CHANGELOG.md | Version history | Medium | 1 hour |

### 4.3 Document Outlines

#### D1: README.md
```markdown
# mdbook-validator

Validate code examples in your mdBook documentation against real tools.

## Features
- SQL validation (SQLite, osquery)
- Config validation (JSON)
- Script validation (bash, python, shellcheck)
- Hidden setup code (readers see clean examples)
- Output assertions (rows, contains, exact match)

## Quick Start

### Installation
cargo install mdbook-validator

### Configuration
Add to book.toml:
[preprocessor.validator]
...

### Usage
Annotate code blocks:
```sql validator=sqlite
SELECT * FROM users;
```

## Requirements
- Docker (running)
- jq (for JSON validators)
- Rust 1.75+ (for building from source)

## Validators
- sqlite: SQLite query validation
- osquery: osquery SQL validation
- osquery-config: osquery JSON config validation
- bash-exec: Bash script execution
- shellcheck: Shell script static analysis
- python: Python syntax validation

## Markers
- <!--SETUP-->: Hidden setup code
- <!--ASSERT-->: Output assertions
- <!--EXPECT-->: Exact output matching
- @@: Hidden context lines

## Examples
[Link to examples directory or inline examples]

## Troubleshooting
See TROUBLESHOOTING.md

## Contributing
See CONTRIBUTING.md

## License
[License info]
```

#### D2: TROUBLESHOOTING.md
```markdown
# Troubleshooting

## Common Errors

### E001: Configuration Error
**Cause**: Invalid book.toml configuration
**Fix**: Check TOML syntax, ensure all required fields present

### E003: Validation Failed
**Cause**: Code block failed validation
**Fixes**:
- Check SETUP block creates required test data
- Verify assertions match actual output
- Run query manually in container to debug

### E005: Container Error
**Cause**: Docker container failed to start or execute
**Fixes**:
- Ensure Docker is running
- Check container image exists and is accessible
- Verify network connectivity for image pull

### E007: Host Validator Error
**Cause**: Validator script failed
**Fixes**:
- Ensure jq is installed
- Check validator script is executable
- Verify script syntax

## Platform-Specific Issues

### Windows
- osqueryi has stdin pipe bug (#7972)
- Workaround: [if available]

### macOS
- Docker Desktop must be running
- Install jq: brew install jq

### Linux
- Docker daemon must be running: systemctl start docker
- Install jq: apt-get install jq

## Performance Issues

### Slow Builds
- Container startup takes 10-20 seconds per validator type
- Containers are cached per build (not across builds)
- Consider grouping validations by validator type

### Memory Usage
- Large JSON outputs loaded into memory
- For outputs > 100MB, consider splitting queries
```

#### D3: VALIDATORS.md
```markdown
# Creating Custom Validators

## Validator Contract

### Input
- **stdin**: JSON output from container command
- **VALIDATOR_ASSERTIONS**: Newline-separated assertion rules
- **VALIDATOR_EXPECT**: Expected output for exact matching
- **VALIDATOR_CONTAINER_STDERR**: Container stderr (for warning detection)

### Output
- **Exit 0**: Validation passed
- **Exit non-zero**: Validation failed
- **stderr**: Error messages shown to user

## Template

See validators/validate-template.sh for a comprehensive template.

## Examples

### Minimal Validator
#!/bin/bash
set -e
JSON_INPUT=$(cat)
echo "$JSON_INPUT" | jq empty || { echo "Invalid JSON" >&2; exit 1; }
exit 0

### With Assertions
[Example with assertion parsing]

### With Expected Output
[Example with exact matching]

## Testing Your Validator

### Manual Testing
echo '{"test": true}' | VALIDATOR_ASSERTIONS="contains test" ./validators/validate-myvalidator.sh

### Integration Testing
[How to add to test suite]

## Registration

Add to book.toml:
[preprocessor.validator.validators.myvalidator]
container = "myimage:1.0"
script = "validators/validate-myvalidator.sh"
exec_command = "mycommand --json"
```

---

## 5. Future Feature Ideas

### 5.1 Feature Candidates

| ID | Feature | Description | Complexity | Value |
|----|---------|-------------|------------|-------|
| F1 | Parallel validation | Run multiple validators concurrently | Medium | High |
| F2 | Block caching | Skip unchanged blocks | Medium | Medium |
| F3 | Watch mode | Re-validate on file change | Low | Medium |
| F4 | GitHub Action | CI integration for validation | Low | High |
| F5 | Snapshot updates | Auto-update EXPECT blocks | Medium | Medium |
| F6 | Custom markers | Configure marker syntax per-project | High | Low |
| F7 | Dry-run mode | Show what would be validated | Low | Medium |
| F8 | Progress output | Show validation progress | Low | Medium |
| F9 | JSON report | Machine-readable validation results | Low | Medium |
| F10 | VSCode extension | Inline validation feedback | High | High |

### 5.2 Feature Details

#### F1: Parallel Validation
**Value**: Faster builds for books with many code blocks
**Approach**:
- Group blocks by validator type (already done for container reuse)
- Run different validator types in parallel
- Requires careful container lifecycle management

#### F4: GitHub Action
**Value**: Easy CI integration
**Approach**:
```yaml
# .github/workflows/validate-docs.yml
name: Validate Documentation
on: [push, pull_request]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: mdbook-validator/action@v1
        with:
          book-dir: ./docs
```

#### F5: Snapshot Updates
**Value**: Easier maintenance of EXPECT blocks
**Approach**:
- New flag: `mdbook-validator --update-snapshots`
- When output differs from EXPECT, update the source file
- Similar to Jest's `--updateSnapshot`

---

## Priority Matrix

### Quick Wins (< 2 hours each)
| ID | Item | Effort | Impact |
|----|------|--------|--------|
| T1 | Consolidate transpiler tests | 30 min | Low |
| T2 | Audit prototype_test.rs | 30 min | Low |
| U1 | Detect jq at startup | 1 hour | Medium |
| U2 | Detect Docker at startup | 1 hour | Medium |
| C4 | Windows documentation | 1 hour | Low |
| D4 | PERFORMANCE.md | 1 hour | Low |
| D5 | CHANGELOG.md | 1 hour | Medium |

### Medium Effort (2-4 hours each)
| ID | Item | Effort | Impact |
|----|------|--------|--------|
| T3 | Validator input validation tests | 2 hours | Medium |
| T4 | shellcheck/python E2E parity | 2 hours | Medium |
| U5 | Validation-only mode | 2 hours | Medium |
| U7-U8 | Error hints and doc links | 2 hours | Medium |
| D1 | README.md | 2 hours | High |
| D2 | TROUBLESHOOTING.md | 2 hours | High |

### Larger Efforts (4+ hours)
| ID | Item | Effort | Impact |
|----|------|--------|--------|
| T5 | Validator script test suite | 4 hours | Medium |
| U3 | `mdbook-validator init` command | 4 hours | High |
| U4 | `mdbook-validator check` command | 3 hours | Medium |
| U6 | Line numbers in errors | 4 hours | Medium |
| U9 | JSON diff output | 3 hours | Low |
| D3 | VALIDATORS.md | 3 hours | Medium |
| C3 | Robust marker parsing | 4 hours | Low |
| C5 | Async cleanup | 4 hours | Low |

### Blocked/Future
| ID | Item | Blocker | Impact |
|----|------|---------|--------|
| C1 | Container pooling | testcontainers-rs #742 | High |
| F1-F10 | Future features | Prioritization needed | Varies |

---

## Recommended Execution Order

### Phase 1: User-Facing Polish (1-2 days)
1. D1: Create README.md
2. D2: Create TROUBLESHOOTING.md
3. U1: Detect jq at startup
4. U2: Detect Docker at startup
5. D5: Create CHANGELOG.md

### Phase 2: Developer Experience (1 day)
1. T1: Consolidate transpiler tests
2. T2: Audit prototype_test.rs
3. T3: Add validator input validation tests
4. T4: Add shellcheck/python E2E test parity

### Phase 3: CLI Improvements (2 days)
1. U3: Add `mdbook-validator init` command
2. U4: Add `mdbook-validator check` command
3. U5: Add validation-only mode
4. U7-U8: Add error hints and doc links

### Phase 4: Advanced Improvements (as needed)
1. U6: Line numbers in errors
2. D3: VALIDATORS.md
3. T5: Validator script test suite
4. Future features based on user feedback

---

## Appendix: Current Test Inventory

| File | Tests | Purpose |
|------|-------|---------|
| src/parser.rs | 35+ | Marker extraction, info string parsing |
| src/transpiler.rs | 15+ | Marker stripping |
| src/config.rs | 15+ | book.toml parsing |
| src/command.rs | 8+ | Command runner |
| tests/integration_tests.rs | 15+ | Full preprocessor flow |
| tests/preprocessor_error_paths.rs | 20+ | Error handling (E001-E010) |
| tests/preprocessor_edge_cases.rs | 12+ | Edge cases |
| tests/e2e_tests.rs | 8+ | End-to-end validation |
| tests/bash_exec_validator_tests.rs | 12+ | bash-exec validator |
| tests/sqlite_validator_tests.rs | 11+ | SQLite validator |
| tests/osquery_validator_tests.rs | 8+ | osquery validator |
| tests/osquery_config_validator_tests.rs | 6+ | osquery config |
| tests/container_tests.rs | 10+ | Container operations |
| tests/docker_mock_tests.rs | 10+ | Docker mocking |
| tests/error_tests.rs | 10+ | Error types |
| tests/config_tests.rs | 13+ | Config parsing |
| tests/parser_tests.rs | 12+ | Parser integration |
| tests/host_validator_tests.rs | 8+ | Host validator |

**Total**: ~155 tests across 18 test files

---

## Appendix: Validator Script Inventory

| Script | Size | Assertions Supported |
|--------|------|---------------------|
| validate-sqlite.sh | 4.8KB | rows, columns, contains, EXPECT |
| validate-osquery.sh | 4.8KB | rows, columns, contains, EXPECT |
| validate-osquery-config.sh | 3.6KB | contains, EXPECT, warning detection |
| validate-bash-exec.sh | 5.5KB | exit_code, stdout_contains, file_exists, dir_exists, file_contains |
| validate-shellcheck.sh | 2.5KB | contains, error detection |
| validate-python.sh | 2.4KB | contains, syntax error detection |
| validate-template.sh | 9.4KB | Template with full documentation |

---

*This plan is a living document. Update as items are completed or priorities change.*
