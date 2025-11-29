# Bash Script Examples

This page demonstrates bash-exec validator examples.

## Simple script (exit 0)

A basic bash script that exits successfully:

```bash validator=bash-exec
#!/bin/bash
echo "Hello from bash"
exit 0
```

## Script with stdout assertion

Validates that the script output contains expected text:

```bash validator=bash-exec
echo "success"
<!--ASSERT
stdout_contains "success"
-->
```

## Script with exit code assertion

Allows non-zero exit codes when explicitly asserted:

```bash validator=bash-exec
exit 42
<!--ASSERT
exit_code = 42
-->
```
