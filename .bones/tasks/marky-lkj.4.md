---
id: marky-lkj.4
title: Implement JSONL parser (line-split + JSON per line)
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


JSONL: each line is a JSON document indexed as [n].key.path. Use line splitting + existing json::parse_json per line. Merge KeyEntry sets with line-prefixed paths.
