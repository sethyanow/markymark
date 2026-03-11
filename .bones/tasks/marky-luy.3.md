---
id: marky-luy.3
title: Evaluate Cow<str> in RealmIndex for reduced allocation on add_document
status: closed
type: task
priority: 3
owner: sethyanow@users.noreply.github.com
parent: marky-luy
---


RealmIndex::add_document does .to_string() on every slug, tag name, and block ID for cross-doc storage. For workspaces with frequent document churn, Cow<'_, str> could avoid allocation when the source document stays indexed. Low priority optimization — measure allocation counts in real corpus benchmarks first.
