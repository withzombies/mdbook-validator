# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.1.2] - 2025-12-02

### Fixed

- **EPIPE handling in command runner**: Fixed race condition where broken pipe errors could cause test failures when scripts exit before stdin is fully written (affects coverage runs on Linux)
  - Note: This fix was added after v1.1.1 was published to crates.io

## [1.1.1] - 2025-12-02

### Fixed

- **Markdown corruption bug**: The `strip_markers_from_chapter` function was reconstructing markdown from pulldown-cmark events, but only handled a subset of events. Lists, blockquotes, links, emphasis, tables, and other elements were silently dropped, causing corrupted output.
  - Rewrote to use span-based editing that surgically modifies only code block contents
  - Added 15 regression tests covering lists, blockquotes, links, inline code, emphasis, tables, headings with links, paths with wildcards, and complex documents

### Added

- **`MDBOOK_LOG` environment variable**: Control log verbosity using the same variable as mdbook itself
  - Supports standard log levels: `error`, `warn`, `info`, `debug`, `trace`
  - Defaults to `info` level when not set
  - Example: `MDBOOK_LOG=debug mdbook build`

## [1.1.0] - 2025-12-02

### Added

- **`hidden` attribute** for code blocks: Validate code examples without showing them to readers
  - Use `validator=sqlite hidden` to validate a block and remove it entirely from output
  - Useful for setup queries, test data, or validation-only examples
  - Hidden blocks are validated (unless `skip` is also present) then completely removed from rendered output
- **E011 error code**: Clear error when `hidden` and `skip` are used together (mutually exclusive)

### Example

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

The first block validates and populates data but is hidden from readers. The second block is shown to readers and validates against the data created by the first.

## [1.0.0] - 2025-11-30

### Added

- Initial release
- Container-based validation using Docker and testcontainers
- Validators: sqlite, osquery, osquery-config, shellcheck, bash-exec, python
- Markers: `<!--SETUP-->`, `<!--ASSERT-->`, `<!--EXPECT-->`
- Line prefix `@@` for hidden context lines
- Assertions: `rows =`, `rows >=`, `contains`, `matches`, `exit_code`, `stdout_contains`, `file_exists`, `dir_exists`, `file_contains`
- Error codes E001-E010 for structured error reporting
- Dependency detection for jq and Docker at startup
- Host-based validator architecture (validators run on host, tools run in containers)

[1.1.0]: https://github.com/withzombies/mdbook-validator/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/withzombies/mdbook-validator/releases/tag/v1.0.0
