---
id: marky-lkj.5
title: Implement flat file parser (.env/.ini/.cfg)
status: closed
type: task
priority: 2
owner: sethyanow@users.noreply.github.com
parent: marky-lkj
---


Hand-rolled key=value parser for .env/.ini/.cfg. Handles # and ; comments, [section] headers for .ini, variable expansion indexed as-is. Depth 0 for .env, depth 0-1 for .ini (sections).
