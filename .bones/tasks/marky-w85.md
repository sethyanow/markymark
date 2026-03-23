---
id: marky-w85
title: 'Test: add non-existent realm error path coverage in runtime_engine tests'
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
---

Copilot finding from PR #28 review.

Current realm tests verify default vs named realm isolation but don't explicitly test the error path when querying a non-existent realm (e.g., realm: Some('does-not-exist'.to_string())).

Add test: pass a realm name that hasn't been initialized and assert the expected structured error is returned rather than panicking or returning empty data.
