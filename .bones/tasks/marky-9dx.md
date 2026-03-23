---
id: marky-9dx
title: Fix pre-existing test_select_binary.sh failures (nested JSON KeyError)
status: closed
type: bug
priority: 3
owner: sethyanow@users.noreply.github.com
---


test_select_binary.sh tests test_lsp_json_uses_plugin_root and test_mcp_json_uses_plugin_root fail because they expect flat JSON (top-level 'command' key) but .lsp.json and .mcp.json use nested structures. The python3 KeyError triggers set -e and aborts the whole test suite after only 3 of 10 tests.
