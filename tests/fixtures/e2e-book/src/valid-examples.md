# Valid Examples

This chapter contains valid code blocks that should pass validation.

## osquery SQL

Query real system tables using osqueryi:

```sql validator=osquery
SELECT uid, username FROM users LIMIT 3;
<!--ASSERT
rows >= 1
-->
```

## SQLite with SETUP

Create a table with test data, then query it:

```sql validator=sqlite
<!--SETUP
sqlite3 /tmp/test.db "CREATE TABLE items (id INTEGER, name TEXT); INSERT INTO items VALUES (1, 'test');"
-->
SELECT * FROM items WHERE id = 1;
<!--ASSERT
rows = 1
contains "test"
-->
```

## Hidden Block (Validated but Not Shown)

This section demonstrates the `hidden` attribute. The following block is validated
but completely removed from output:

```sql validator=sqlite hidden
<!--SETUP
sqlite3 /tmp/test.db "CREATE TABLE hidden_test (id INTEGER, marker TEXT); INSERT INTO hidden_test VALUES (1, 'XYZ_HIDDEN_BLOCK_CONTENT_789');"
-->
SELECT marker FROM hidden_test WHERE marker = 'XYZ_HIDDEN_BLOCK_CONTENT_789';
<!--ASSERT
rows = 1
contains "XYZ_HIDDEN_BLOCK_CONTENT_789"
-->
```

The query below uses the table created by the hidden block above:

```sql validator=sqlite
SELECT COUNT(*) as count FROM hidden_test;
<!--ASSERT
rows = 1
-->
```

## osquery Config (JSON)

Valid osquery configuration file:

```json validator=osquery-config
{
  "options": {
    "logger_path": "/var/log/osquery"
  }
}
```

## Shellcheck (Static Analysis)

Valid shell script with properly quoted variables:

```bash validator=shellcheck
#!/bin/bash
# Valid script - variables properly quoted
name="world"
echo "Hello, $name"
```

## Python (Static Analysis)

Valid Python script with correct syntax:

```python validator=python
def hello(name):
    """Greet someone."""
    return f"Hello, {name}!"

if __name__ == "__main__":
    print(hello("world"))
```
