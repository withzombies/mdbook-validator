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
