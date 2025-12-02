# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
