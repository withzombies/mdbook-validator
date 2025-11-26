//! Tests for marker stripping functionality

use mdbook_validator::transpiler::strip_markers;

#[test]
fn strip_markers_removes_setup_block() {
    let input = r"<!--SETUP
CREATE TABLE test (id INTEGER);
INSERT INTO test VALUES (1);
-->
SELECT * FROM test;";

    let expected = "SELECT * FROM test;";
    assert_eq!(strip_markers(input), expected);
}

#[test]
fn strip_markers_removes_assert_block() {
    let input = r"SELECT COUNT(*) as total FROM test
<!--ASSERT
rows = 1
total = 3
-->";

    let expected = "SELECT COUNT(*) as total FROM test";
    assert_eq!(strip_markers(input), expected);
}

#[test]
fn strip_markers_removes_expect_block() {
    let input = r#"SELECT id FROM test ORDER BY id
<!--EXPECT
[{"id": 1}, {"id": 2}]
-->"#;

    let expected = "SELECT id FROM test ORDER BY id";
    assert_eq!(strip_markers(input), expected);
}

#[test]
fn strip_markers_removes_double_at_lines() {
    let input = r"@@watch_paths = ['/home/%%']
@@exclude_paths = []
@@
[policies]
enabled_policies = ['ccpa']
@@
@@[work_queue]
@@max_queue_size = 10000";

    let expected = r"[policies]
enabled_policies = ['ccpa']";
    assert_eq!(strip_markers(input), expected);
}

#[test]
fn strip_markers_handles_all_marker_types_together() {
    let input = r"<!--SETUP
CREATE TABLE alerts (path TEXT, scanner TEXT);
INSERT INTO alerts VALUES ('/data/test.json', 'scanner1');
-->
@@-- This comment is hidden
SELECT path FROM alerts WHERE path LIKE '%.json'
<!--ASSERT
rows >= 1
contains 'test.json'
-->";

    let expected = "SELECT path FROM alerts WHERE path LIKE '%.json'";
    assert_eq!(strip_markers(input), expected);
}
