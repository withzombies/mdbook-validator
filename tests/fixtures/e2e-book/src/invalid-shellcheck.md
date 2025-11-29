# Invalid Shellcheck Example

This script has SC2086 (unquoted variable) which shellcheck will catch.
Using `cat $file` triggers the warning because word splitting could break on spaces:

```bash validator=shellcheck
#!/bin/bash
file="test file.txt"
cat $file
```
