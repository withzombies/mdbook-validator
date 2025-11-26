# Documentation Validation Landscape Analysis

**Date**: 2025-11-25
**Purpose**: Identify design patterns and potential flaws in mdbook-validator by studying existing documentation validation tools
**Scope**: 15+ tools and approaches from industry leaders (Stripe, Twilio, Google) and open-source projects

---

## Executive Summary

Documentation code examples rot. Every major documentation system has evolved solutions for validating that code examples actually work. This analysis examines 15+ approaches to identify patterns that mdbook-validator should adopt and pitfalls to avoid.

**Key findings**:
1. Output matching is harder than simple string comparison - normalization is essential
2. "Compile but don't run" modes are table stakes for network/destructive examples
3. Parallel execution is expected, not optional
4. Line numbers in error messages are critical for user experience
5. TAP output format enables CI integration across ecosystems

---

## Tools and Approaches Surveyed

### Comparison Matrix

| Tool/System | Language | Annotation Style | Output Validation | Parallel | Container |
|-------------|----------|------------------|-------------------|----------|-----------|
| **Rust doctests** | Rust | Doc comments | Implicit (panic=fail) | Yes | No |
| **Python doctest** | Python | `>>>` REPL prompts | Exact stdout match | No | No |
| **Sphinx doctest** | Python | RST directives | Output comparison | No | No |
| **txm** | Any | HTML comments | Named in/out pairs | Yes | No |
| **nbval** | Jupyter | Cell markers | Cell output diff | Yes* | No |
| **Twilio api-snippets** | Multi | meta.json/test.yml | Mock API + compare | Yes | No |
| **pgTAP** | PostgreSQL | SQL functions | TAP assertions | N/A | No |
| **readme-to-test** | JavaScript | Code fence extraction | Mocha assertions | Yes | No |
| **runmd** | JavaScript | Special syntax | Stdout capture | No | No |
| **mdbook-cmdrun** | Any | `<!-- cmdrun -->` | None (display only) | No | No |
| **byexample** | Multi | `>>>` style prompts | Output comparison | No | No |
| **embedme** | Any | File references | Verify flag | No | No |
| **Doc Detective** | Any | JSON test specs | UI/API assertions | Yes | No |
| **Papermill** | Jupyter | Cell parameters | Notebook diffing | Yes | No |
| **mdbook-validator** | Any | HTML comments + `@@` | JSON + assertions | **No** | **Yes** |

*nbval supports parallel via pytest-xdist with `--dist loadscope`

### Key Observations

1. **mdbook-validator is unique in using containers** - Every other tool runs code in the host environment. This is a significant differentiator for isolation and reproducibility.

2. **Most tools lack structured output** - They compare raw stdout, leading to brittleness. mdbook-validator's JSON output from validators is more robust.

3. **Parallel execution is common** - txm, nbval, Twilio, readme-to-test all support parallel. mdbook-validator's sequential design is an outlier.

---

## Detailed Tool Analysis

### 1. Rust Doctests

**Source**: https://doc.rust-lang.org/rustdoc/documentation-tests.html

Rust's approach is the gold standard for compiled languages. Key features:

```rust
/// Returns the sum of two numbers
///
/// # Examples
///
/// ```
/// let result = mylib::add(2, 3);
/// assert_eq!(result, 5);
/// ```
pub fn add(a: i32, b: i32) -> i32 { a + b }
```

**Annotations supported**:
- `ignore` - Don't compile or run
- `no_run` - Compile but don't execute
- `should_panic` - Expect a panic
- `compile_fail` - Expect compilation failure

**Hidden lines**: Lines starting with `#` are compiled but hidden from rendered docs.

**Relevance to mdbook-validator**:
- The `no_run` equivalent is missing from our design
- Hidden line syntax (`#`) inspired our `@@` prefix
- Assertion-based validation (not output comparison) is more robust

---

### 2. Python doctest

**Source**: https://docs.python.org/3/library/doctest.html

The original documentation testing tool. Uses REPL-style prompts:

```python
def factorial(n):
    """Return the factorial of n.

    >>> factorial(5)
    120
    >>> factorial(0)
    1
    """
    if n == 0:
        return 1
    return n * factorial(n-1)
```

**Known limitations** (directly relevant to mdbook-validator):
- **Whitespace sensitivity**: Extra blank lines break tests
- **Output order**: Dictionary output order can vary (fixed in Python 3.7+)
- **Floating point**: `0.1 + 0.2` outputs `0.30000000000000004`
- **Exception formatting**: Tracebacks differ between Python versions

**Special directives**:
- `# doctest: +ELLIPSIS` - Allow `...` wildcards
- `# doctest: +NORMALIZE_WHITESPACE` - Collapse whitespace
- `# doctest: +SKIP` - Skip this example
- `<BLANKLINE>` - Explicit blank line marker

**Relevance to mdbook-validator**:
- Our `<!--EXPECT-->` exact matching will have the same brittleness
- We need normalization options (`normalize=json,whitespace`)
- The `+ELLIPSIS` pattern (wildcards) is worth adopting

---

### 3. Sphinx doctest Extension

**Source**: https://www.sphinx-doc.org/en/master/usage/extensions/doctest.html

Sphinx adds structured directives for documentation testing:

```rst
.. testsetup::

   import mymodule

.. doctest::

   >>> mymodule.foo()
   'bar'

.. testcleanup::

   mymodule.cleanup()
```

**Key features**:
- `testsetup` / `testcleanup` - Shared setup across multiple blocks
- `testcode` / `testoutput` - Separate code from expected output
- Groups - Organize related tests

**Relevance to mdbook-validator**:
- Our lack of shared setup (`testsetup` equivalent) will frustrate users
- The separation of code and output into different blocks is cleaner than inline markers

---

### 4. txm (Language-Agnostic Markdown Tester)

**Source**: https://github.com/anko/txm

txm is the closest analog to mdbook-validator's goals. Uses HTML comments:

```markdown
<!-- !test program node -->

<!-- !test in example -->
```js
console.log(2 + 2);
```

<!-- !test out example -->
```
4
```
```

**Key features**:
- **Named test pairs**: Input and output linked by name
- **Parallel execution**: `--jobs N` flag, default parallel
- **TAP output**: Standard test protocol format
- **Rich diagnostics**: Diffs, line numbers, invisible character visualization
- **Self-testing**: txm's own README is tested with txm

**Relevance to mdbook-validator**:
- Named pairs allow testing same code with different expected outputs
- TAP format enables CI integration
- Invisible character visualization (null becomes `␀`) aids debugging
- Parallel execution is default, not optional

---

### 5. nbval (Jupyter Notebook Validation)

**Source**: https://github.com/computationalmodelling/nbval

Validates Jupyter notebooks by comparing cell outputs:

```python
# NBVAL_IGNORE_OUTPUT
import random
print(random.random())  # Output varies, but cell should run
```

**Cell annotations**:
- `# NBVAL_IGNORE_OUTPUT` - Run but don't check output
- `# NBVAL_SKIP` - Don't execute this cell
- `# NBVAL_RAISES_EXCEPTION` - Expect an exception
- `# NBVAL_CHECK_OUTPUT` - Force output checking (with `--nbval-lax`)

**Relevance to mdbook-validator**:
- The ignore/skip/raises pattern is comprehensive
- We're missing "expect error" functionality
- Lax mode (run without checking output) is useful for CI smoke tests

---

### 6. Twilio api-snippets

**Source**: https://github.com/TwilioDevEd/api-snippets

Twilio tests ~20,000 code samples across 9 languages. Their approach:

**Testing strategy**:
1. **API/REST snippets**: Syntax check + mock API server
2. **TwiML snippets**: Capture stdout, compare against expected XML

**Infrastructure**:
- Custom "twilio-api-faker" mock server
- Ruby-based test runner (`snippet_tester.rb`)
- Travis CI integration
- `meta.json` or `test.yml` files mark testable snippets

**Relevance to mdbook-validator**:
- Mock server pattern valuable for API documentation
- Metadata files (`meta.json`) separate test config from content
- Multi-language support requires language-specific tooling

---

### 7. pgTAP (PostgreSQL Unit Testing)

**Source**: https://pgtap.org/documentation.html

Database-native testing framework using TAP protocol:

```sql
BEGIN;
SELECT plan(2);

SELECT has_table('users');
SELECT has_column('users', 'email');

SELECT * FROM finish();
ROLLBACK;
```

**Key features**:
- Tests run inside transactions (automatic rollback)
- Rich assertion library (`has_table`, `col_is_pk`, `row_eq`, etc.)
- `runtests()` for xUnit-style test discovery
- Setup/teardown functions

**Relevance to mdbook-validator**:
- Transaction wrapping ensures test isolation (we have container isolation)
- Domain-specific assertions are more useful than generic output comparison
- The assertion vocabulary (`has_table`, `col_is_pk`) is worth studying for SQL validators

---

### 8. Google Cloud Samples Style Guide

**Source**: https://googlecloudplatform.github.io/samples-style-guide/

Google's requirements for code samples:

**Core principles**:
1. **Copy-paste-runnable**: Users can run with minimal changes
2. **Teach through code**: Show best practices, not just functionality
3. **Idiomatic**: Follow language conventions
4. **Low cyclomatic complexity**: Single code path per example

**Testing requirements**:
- All samples must have automated tests
- Tests should cover both success and error paths
- Samples should demonstrate error handling

**Relevance to mdbook-validator**:
- "Copy-paste-runnable" aligns with validation goals
- Error handling examples need "expect error" support
- Low complexity suggests our examples should be small (supports 30KB limit)

---

## Design Flaws Exposed

### 1. Output Matching Is Harder Than Expected

**Problem**: `<!--EXPECT-->` does exact JSON matching.

**Real-world issues**:
- Whitespace variations in JSON formatting
- Non-deterministic values (timestamps, UUIDs)
- Floating-point representation differences
- Key ordering in objects (JSON doesn't guarantee order)

**Evidence**: Python doctest requires `+NORMALIZE_WHITESPACE`, `+ELLIPSIS`. nbval has `NBVAL_IGNORE_OUTPUT`.

**Recommendation**: Add normalization options:
```markdown
<!--EXPECT normalize=json,whitespace ignore=timestamps
{"id": 1, "created_at": "..."}
-->
```

Or use JSON schema validation instead of exact matching:
```markdown
<!--ASSERT
json_schema {"type": "object", "required": ["id", "name"]}
-->
```

---

### 2. Missing "Compile But Don't Run" Mode

**Problem**: `skip` skips entirely. No middle ground.

**Use cases requiring syntax-only validation**:
- Network examples ("fetch this URL")
- Destructive operations ("delete all files")
- Examples requiring external state
- Performance-heavy operations

**Evidence**: Rust doctests have `no_run`. Sphinx has `testcode` without `testoutput`.

**Recommendation**: Add `syntax` annotation:
```markdown
```sql validator=osquery syntax
-- Validates SQL syntax without executing
SELECT * FROM future_table;
```
```

---

### 3. No Setup Sharing Across Blocks

**Problem**: Every block needs its own `<!--SETUP-->`.

**Impact**: A chapter with 20 SQL examples using the same schema duplicates setup 20 times.

**Evidence**: Sphinx has `testsetup` applying to multiple blocks. pgTAP has shared setup functions.

**Recommendation** (v1.1): File-level or chapter-level setup:
```toml
# book.toml
[preprocessor.validator.setup.sqlite]
applies-to = "src/sql-chapter/*.md"
content = """
CREATE TABLE users (id INTEGER, name TEXT);
"""
```

---

### 4. Error Messages Lack Line Numbers

**Problem**: "Error in src/chapter.md" doesn't say where.

**Evidence**: txm outputs line numbers. Python doctest shows exact source location.

**Technical note**: We already use `into_offset_iter()` for byte ranges. Computing line numbers is trivial:
```rust
fn byte_offset_to_line(source: &str, offset: usize) -> usize {
    source[..offset].matches('\n').count() + 1
}
```

**Recommendation**: Always include line numbers:
```
Error: Validation failed in src/network-queries.md:47

  | ```sql validator=osquery
  | SELECT local_port FROM listening_ports;
  | ```

Validator stderr:
  Error: no such table: listening_ports
```

---

### 5. Sequential Execution Is Too Slow

**Problem**: Design validates blocks sequentially.

**Math**: 50 blocks × ~3 minutes = acceptable. 200 blocks × ~12 minutes = unacceptable.

**Evidence**: txm defaults to parallel (`--jobs`). nbval supports `pytest-xdist`. Twilio runs parallel in CI.

**Recommendation**: Parallelize by validator type (same container, concurrent execs):
```rust
// All sqlite blocks share one container, run concurrently
let sqlite_results = futures::future::join_all(
    sqlite_blocks.iter().map(|b| runner.execute(&sqlite_container, b))
).await;
```

---

### 6. No TAP Output Format

**Problem**: Human-readable output only.

**Impact**: Harder to integrate with CI systems, test aggregators, dashboards.

**Evidence**: pgTAP, txm both output TAP. It's a 40-year-old standard.

**Recommendation**: Add `--format tap` option:
```
TAP version 13
1..5
ok 1 - src/chapter1.md:15 sql validator=sqlite
ok 2 - src/chapter1.md:42 sql validator=sqlite
not ok 3 - src/chapter2.md:18 sql validator=osquery
  ---
  message: 'no such table: listening_ports'
  severity: fail
  ...
ok 4 - src/chapter2.md:55 json validator=osquery-config
ok 5 - src/chapter3.md:12 bash validator=shellcheck
```

---

### 7. No "Expect Error" Mode

**Problem**: Can't document intentionally failing examples.

**Use cases**:
- "This query fails because..."
- "Common mistakes to avoid"
- Error message documentation

**Evidence**: Rust has `should_panic`, `compile_fail`. nbval has `NBVAL_RAISES_EXCEPTION`.

**Recommendation**: Add `expect-error` annotation:
```markdown
```sql validator=sqlite expect-error="no such table"
SELECT * FROM nonexistent;
```
```

---

### 8. No Partial Output Matching

**Problem**: Only `contains` (substring) or `<!--EXPECT-->` (exact).

**Missing capability**: Assert output structure without exact values.

**Example need**: "Output is a JSON array with objects containing `id` and `name` fields, but I don't care about specific values."

**Recommendation**: JSON path assertions:
```markdown
<!--ASSERT
json_path "$[*].id" exists
json_path "$[0].name" typeof string
json_length "$" >= 1
-->
```

---

## Strengths of mdbook-validator Design

### 1. Container Isolation (Unique)

Every other tool runs code in the host environment. Container isolation provides:
- Reproducible environments across machines
- No host system pollution
- Specific tool versions (osquery 5.12.1, not "whatever's installed")
- Security isolation for untrusted examples

### 2. Hidden Context Lines (`@@` Prefix)

Python doctest's `#` hiding is Python-specific. Our `@@` works for any language:
```toml
@@[required_section]
@@key = "value"
[visible_section]
setting = true
```

This enables validating complete configs while showing only relevant portions.

### 3. Separated Assertions from Output

Most tools use "expected output IS the assertion." Our separation is cleaner:
- `<!--ASSERT-->` for validation logic (rows >= 1, contains "x")
- `<!--EXPECT-->` for regression testing (exact output match)

This allows flexible validation without exact output matching.

### 4. Structured Validator Output

Validators return JSON with typed fields:
```json
{"success": true, "output": "...", "row_count": 2, "error": null}
```

This avoids the output-parsing fragility that plagues doctest-style tools.

### 5. Validator-Interpreted Setup

`<!--SETUP-->` meaning is validator-specific. SQLite treats it as SQL; bash-exec treats it as shell commands. This is more flexible than generic "run before" semantics.

---

## Revised Implementation Priority

Based on this analysis:

### Phase 1: Core MVP (as planned)
- Basic validation loop
- osquery SQL validator
- Marker extraction and stripping

### Phase 1.5: Critical UX (add before release)
- **Line numbers in error messages** - Users will demand immediately
- **`syntax` annotation** - For examples that shouldn't run

### Phase 2: Performance (required for adoption)
- **Parallel execution per validator type** - Non-negotiable for large books
- **Container pooling improvements**

### Phase 3: Output Handling (before `<!--EXPECT-->` is used)
- **Output normalization options** - Prevent brittleness
- **JSON schema assertion** - Flexible structure validation

### Phase 4: CI Integration
- **TAP output format** - Standard test protocol
- **`--format json`** - Machine-readable results
- **Exit codes** - Proper CI signaling

### Phase 5: Advanced Features
- **`expect-error` annotation** - Document failures
- **Shared setup blocks** - Reduce duplication
- **JSON path assertions** - Flexible validation

---

## References

### Primary Sources
- [Rust doctests](https://doc.rust-lang.org/rustdoc/documentation-tests.html)
- [Python doctest](https://docs.python.org/3/library/doctest.html)
- [Sphinx doctest extension](https://www.sphinx-doc.org/en/master/usage/extensions/doctest.html)
- [txm - Markdown code example tester](https://github.com/anko/txm)
- [nbval - Jupyter notebook validation](https://github.com/computationalmodelling/nbval)
- [Twilio api-snippets](https://github.com/TwilioDevEd/api-snippets)
- [pgTAP - PostgreSQL testing](https://pgtap.org/documentation.html)

### Secondary Sources
- [Google Cloud Samples Style Guide](https://googlecloudplatform.github.io/samples-style-guide/)
- [readme-to-test](https://github.com/aswitalski/readme-to-test)
- [runmd](https://github.com/broofa/runmd)
- [Papermill](https://papermill.readthedocs.io/)
- [nbval documentation](https://nbval.readthedocs.io/en/latest/index.html)
- [Real Python doctest tutorial](https://realpython.com/python-doctest/)
- [Doc Detective](https://www.docsastests.com/ci-with-github-actions)

### Background Reading
- [Docs-as-Code CI/CD workflow](https://pronovix.com/blog/cicd-and-docs-code-workflow)
- [Literate Programming](http://www.literateprogramming.com/)
- [TAP - Test Anything Protocol](https://testanything.org/)
