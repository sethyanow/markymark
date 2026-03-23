---
id: marky-g9t.5
title: Update RealmIndex for hybrid arena model
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
depends_on: [marky-g9t.4, marky-luy]
parent: marky-g9t
---





Update markymark-index/src/realm.rs to store documents with their arenas (each doc owns its Bump + DocumentIndex<'arena>). Cross-doc lookups (slug→heading, tag aggregation) use owned String copies since they need to outlive individual document arenas. Update add_document/remove_document to handle arena lifecycle.

Success: cargo test -p markymark-index passes. Realm tests verify doc add/remove/re-parse.
