---
id: marky-lkj.8
title: 'Cross-doc resolution: markdown [[file#key.path]] to structured doc keys'
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


Extend resolution module to resolve wiki-links targeting structured doc key paths. [[config.json#database.host]] should resolve to the key entry in the JSON file. Update find-references to work bidirectionally. Requires extending ResolvedTarget enum.
