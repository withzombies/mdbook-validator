# Troubleshooting

This guide covers common errors and platform-specific issues when using mdbook-validator.

## Quick Reference

| Code | Error Type | Quick Fix |
|------|------------|-----------|
| E001 | Configuration | Check book.toml syntax and structure |
| E002 | Container Startup | Ensure Docker is running and image exists |
| E003 | Container Exec | Check command syntax and container health |
| E004 | Setup Failed | Fix SETUP block script errors |
| E005 | Query Failed | Fix visible content (SQL, script) errors |
| E006 | Validation Failed | Fix assertion mismatches or output issues |
| E007 | Unknown Validator | Add validator to book.toml config |
| E008 | Invalid Config | Add required fields (container, script) |
| E009 | Fixtures Error | Check fixtures_dir path exists and is a directory |
| E010 | Script Not Found | Check validator script path is correct |
| E011 | Mutually Exclusive | Remove either `hidden` or `skip` (can't use both) |

---

## Error Reference

### E001: Configuration Error

**Message**: `[E001] Configuration error: {message}`

**Common Causes**:
- Invalid TOML syntax in book.toml
- Missing `[preprocessor.validator]` section
- Typo in configuration key names

**How to Fix**:
1. Validate your TOML syntax:
   ```bash
   # Check for syntax errors
   cat book.toml | python3 -c "import tomllib, sys; tomllib.load(sys.stdin.buffer)"
   ```
2. Ensure the basic structure exists:
   ```toml
   [preprocessor.validator]
   command = "mdbook-validator"
   ```
3. Check for common typos: `validators` not `validator`, `container` not `containers`

**Example**:
```
[E001] Configuration error: expected `=`, found newline
```
Fix: Check for missing `=` in your book.toml key-value pairs.

---

### E002: Container Startup Failed

**Message**: `[E002] Container startup failed: {message}`

**Common Causes**:
- Docker daemon not running
- Container image doesn't exist or can't be pulled
- Network connectivity issues preventing image download
- Invalid image tag format

**How to Fix**:
1. Verify Docker is running:
   ```bash
   docker info
   ```
   If this fails, start Docker Desktop (macOS/Windows) or the Docker daemon (Linux).

2. Test that the image can be pulled:
   ```bash
   docker pull keinos/sqlite3:3.47.2  # Replace with your image
   ```

3. Check your network connection if the pull fails.

4. Verify the image tag is valid (avoid `:latest`, use specific versions).

**Example**:
```
[E002] Container startup failed: image not found: badimage:999
```
Fix: Check the `container` value in your validator config matches an existing Docker image.

---

### E003: Container Exec Failed

**Message**: `[E003] Container exec failed: {message}`

**Common Causes**:
- Command doesn't exist in the container
- Container exited unexpectedly
- Timeout waiting for container response
- Permission denied inside container

**How to Fix**:
1. Test the command manually:
   ```bash
   docker run --rm keinos/sqlite3:3.47.2 sqlite3 --version
   ```

2. Check that the `exec_command` in your config is correct:
   ```toml
   [preprocessor.validator.validators.sqlite]
   container = "keinos/sqlite3:3.47.2"
   script = "validators/validate-sqlite.sh"
   exec_command = "sqlite3 -json /tmp/test.db"  # Verify this command exists
   ```

3. Check container logs if available.

**Example**:
```
[E003] Container exec failed: command not found: osqueryi
```
Fix: Ensure the container image includes the required tool, or fix the command name.

---

### E004: Setup Failed

**Message**: `[E004] Setup script failed (exit {exit_code}): {message}`

**Common Causes**:
- Invalid SQL in SETUP block
- Shell syntax error in SETUP script
- Missing prerequisites (table doesn't exist, file not found)
- Permission denied

**How to Fix**:
1. Test your SETUP content manually:
   ```bash
   # For SQLite
   sqlite3 /tmp/test.db "YOUR SETUP SQL HERE"

   # For bash
   bash -c "YOUR SETUP SCRIPT HERE"
   ```

2. Check for syntax errors in your SETUP block:
   ```markdown
   <!--SETUP
   CREATE TABLE test (id INTEGER);  -- Valid SQL
   -->
   ```

3. Ensure SETUP runs before the visible content depends on it.

**Example**:
```
[E004] Setup script failed (exit 1): Error: near "CREAT": syntax error
```
Fix: Correct the typo (`CREAT` → `CREATE`) in your SETUP block.

---

### E005: Query Failed

**Message**: `[E005] Query execution failed (exit {exit_code}): {message}`

**Common Causes**:
- SQL syntax error in the visible content
- Table/column referenced doesn't exist (SETUP may be missing or failed)
- Invalid command for the validator type
- Empty content after marker stripping

**How to Fix**:
1. Test your query manually:
   ```bash
   # Run setup first, then query
   sqlite3 /tmp/test.db "CREATE TABLE t(id INT)"
   sqlite3 -json /tmp/test.db "SELECT * FROM t"
   ```

2. Ensure SETUP creates required tables/data before the query runs.

3. Check that visible content isn't empty after stripping markers.

**Example**:
```
[E005] Query execution failed (exit 1): Error: no such table: users
```
Fix: Add a SETUP block that creates the `users` table before the query.

---

### E006: Validation Failed

**Message**: `[E006] Validation failed (exit {exit_code}): {message}`

**Common Causes**:
- Assertion doesn't match actual output (e.g., `rows = 5` but query returns 3)
- EXPECT block doesn't match actual JSON output
- Validator script logic rejected the output
- Contains assertion failed (string not found in output)

**How to Fix**:
1. Check your assertions match expected output:
   ```markdown
   <!--ASSERT
   rows = 1        -- Exact row count
   rows >= 1       -- Minimum row count
   contains "alice" -- String must appear in output
   -->
   ```

2. For EXPECT blocks, ensure JSON matches exactly:
   ```markdown
   <!--EXPECT
   [{"id": 1, "name": "alice"}]
   -->
   ```

3. Run the query manually to see actual output:
   ```bash
   sqlite3 -json /tmp/test.db "SELECT * FROM users"
   ```

4. Update assertions to match actual behavior.

**Example**:
```
[E006] Validation failed (exit 1): Assertion failed: rows = 5 (actual: 1)
```
Fix: Change `rows = 5` to `rows = 1` or fix your SETUP to insert 5 rows.

---

### E007: Unknown Validator

**Message**: `[E007] Unknown validator '{name}'`

**Common Causes**:
- Typo in validator name (`validtor=sqlite` instead of `validator=sqlite`)
- Validator not configured in book.toml
- Using a validator name that doesn't exist

**How to Fix**:
1. Check spelling in your markdown:
   ```markdown
   ```sql validator=sqlite   <!-- Correct -->
   ```sql validtor=sqlite    <!-- Typo! -->
   ```

2. Add the validator to book.toml:
   ```toml
   [preprocessor.validator.validators.sqlite]
   container = "keinos/sqlite3:3.47.2"
   script = "validators/validate-sqlite.sh"
   ```

3. Ensure validator names match between markdown and config.

**Example**:
```
[E007] Unknown validator 'sqllite'
```
Fix: Correct the typo in your markdown (`sqllite` → `sqlite`).

---

### E008: Invalid Validator Config

**Message**: `[E008] Invalid validator config for '{name}': {reason}`

**Common Causes**:
- Missing `container` field
- Missing `script` field
- Empty values for required fields
- Invalid path format

**How to Fix**:
1. Ensure both required fields are present:
   ```toml
   [preprocessor.validator.validators.myvalidator]
   container = "myimage:1.0"    # Required
   script = "validators/my.sh"  # Required
   exec_command = "..."         # Optional
   ```

2. Don't use empty strings:
   ```toml
   container = ""  # Invalid - must have a value
   ```

3. Check path separators match your OS (use forward slashes).

**Example**:
```
[E008] Invalid validator config for 'sqlite': container cannot be empty
```
Fix: Add a valid container image name to the validator config.

---

### E009: Fixtures Error

**Message**: `[E009] Fixtures directory error: {message}`

**Common Causes**:
- `fixtures_dir` path doesn't exist
- Path exists but is a file, not a directory
- Permission denied accessing the directory
- Relative path resolved incorrectly

**How to Fix**:
1. Check the path exists:
   ```bash
   ls -la fixtures/  # Or your configured path
   ```

2. Ensure it's a directory, not a file:
   ```bash
   test -d fixtures && echo "OK: is a directory"
   ```

3. Use relative paths from book root, or absolute paths:
   ```toml
   [preprocessor.validator]
   fixtures_dir = "fixtures"  # Relative to book.toml location
   ```

**Example**:
```
[E009] Fixtures directory error: 'fixtures' does not exist
```
Fix: Create the directory or update the path in book.toml.

---

### E010: Script Not Found

**Message**: `[E010] Script not found: {path}`

**Common Causes**:
- Validator script file doesn't exist at specified path
- Path typo in book.toml
- Script was deleted or moved
- Wrong relative path base

**How to Fix**:
1. Check the script exists:
   ```bash
   ls -la validators/validate-sqlite.sh
   ```

2. Ensure the script is executable:
   ```bash
   chmod +x validators/validate-sqlite.sh
   ```

3. Verify the path in book.toml is correct (relative to book root):
   ```toml
   [preprocessor.validator.validators.sqlite]
   script = "validators/validate-sqlite.sh"  # Relative to book.toml
   ```

4. Copy validator scripts from the mdbook-validator package if missing.

**Example**:
```
[E010] Script not found: validaters/validate-sqlite.sh
```
Fix: Correct the typo in the path (`validaters` → `validators`).

---

### E011: Mutually Exclusive Attributes

**Message**: `[E011] 'hidden' and 'skip' are mutually exclusive`

**Common Causes**:
- Code block has both `hidden` and `skip` attributes
- Copy-paste error from another block
- Confusion about what each attribute does

**How to Fix**:
1. Understand the difference:
   - `skip` = Don't validate this block, but show it to readers
   - `hidden` = Validate this block, but don't show it to readers

2. Choose one based on your intent:
   ```markdown
   <!-- If you want to show an intentionally broken example (no validation): -->
   ```sql validator=sqlite skip
   SELECT * FROM nonexistent_table;
   ```

   <!-- If you want to validate but hide from readers: -->
   ```sql validator=sqlite hidden
   INSERT INTO users VALUES (1, 'alice');
   ```
   ```

3. Remove the attribute you don't need.

**Example**:
```
[E011] 'hidden' and 'skip' are mutually exclusive
```
Fix: Remove either `hidden` or `skip` from your code block attributes.

---

## Platform-Specific Issues

### macOS

**Docker Desktop Required**:
- Install Docker Desktop from https://docker.com
- Ensure Docker Desktop is running (check menu bar icon)
- Grant necessary permissions when prompted

**Installing jq**:
```bash
brew install jq
```

**Common Issues**:
- "Cannot connect to Docker daemon": Start Docker Desktop application
- Slow first runs: Docker is pulling images (10-30 seconds per validator type)

---

### Linux

**Docker Daemon**:
```bash
# Start Docker daemon
sudo systemctl start docker

# Enable Docker on boot
sudo systemctl enable docker

# Run Docker without sudo (optional)
sudo usermod -aG docker $USER
# Log out and back in for this to take effect
```

**Installing jq**:
```bash
# Debian/Ubuntu
sudo apt-get install jq

# Fedora
sudo dnf install jq

# Arch
sudo pacman -S jq
```

**Common Issues**:
- "Permission denied": Add user to docker group or use sudo
- "Docker daemon not running": Run `systemctl start docker`

---

### Windows

**Docker Desktop Required**:
- Install Docker Desktop from https://docker.com
- Enable WSL 2 backend (recommended) or Hyper-V
- Ensure Docker Desktop is running

**Installing jq**:
```powershell
# Using Chocolatey
choco install jq

# Using Scoop
scoop install jq
```

**Known Limitation - osqueryi stdin bug**:
osqueryi has a known issue (#7972) where stdin piping fails on Windows with "incomplete SQL" errors. Workaround: Use WSL 2 or run validation on Linux/macOS.

---

## Performance Tips

### Container Startup Time

First validation for each validator type takes 10-20 seconds while the container starts. Subsequent validations in the same build reuse the running container.

**Tip**: Group your validated code blocks by validator type to minimize container restarts.

### Image Caching

Docker caches images locally. The first build pulls images (slow), subsequent builds reuse cached images (fast).

**Pre-pull images** before building:
```bash
docker pull keinos/sqlite3:3.47.2
docker pull osquery/osquery:5.17.0-ubuntu22.04
docker pull koalaman/shellcheck-alpine:stable
```

### Large Books

For books with many validated code blocks:
1. Use `fail-fast = true` (default) to stop on first error during development
2. Set `fail-fast = false` in CI to see all errors at once
3. Consider splitting very large books into multiple builds

### Memory Usage

Large JSON outputs are loaded into memory. For queries returning >100MB of data:
- Add `LIMIT` clauses to SQL queries
- Split into multiple smaller queries
- Consider if the full output is necessary for validation

---

## Getting Help

If you encounter an error not covered here:

1. Check the error code (E001-E011) for category
2. Run with `RUST_LOG=debug mdbook build` for verbose output
3. Open an issue at https://github.com/withzombies/mdbook-validator/issues

Include in your report:
- Error message (full text)
- Relevant book.toml configuration
- Code block that caused the error
- OS and Docker version
