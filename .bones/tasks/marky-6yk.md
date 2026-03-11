---
id: marky-6yk
title: 'Task 5: Implement graph-analysis MCP tool'
status: closed
type: feature
priority: 1
owner: sethyanow@users.noreply.github.com
parent: marky-9mo
---


Implement the graph-analysis MCP tool required by epic marky-9mo. This tool provides workspace-wide link graph intelligence: orphan detection, hub detection, broken link report, cluster analysis, and summary stats.

## Design

## Goal
Implement graph-analysis MCP tool exposing link graph intelligence over the indexed markdown workspace.

## Context
All child tasks of marky-9mo are closed except this one. The ConnectionGraph and RealmIndex already provide the data needed. DependencyGraph operation exists for raw graph export; graph-analysis is a higher-level analysis layer.

## Architecture
- New CoreOperation::GraphAnalysis { realm, include_clusters: bool, top_n_hubs: u32 }
- New CoreOperationResult::GraphAnalysis { ... } with analysis payload struct
- New graph_analysis function in markymark-mcp/src/runtime_engine.rs
- New graph_analysis_tool in markymark-mcp/src/lib.rs
- New DTO types in markymark-mcp/src/dto.rs

## Analysis Features

### Orphan Detection
Documents with BOTH zero incoming AND zero outgoing references within the workspace (wiki links and local markdown links). External HTTP links are excluded.

### Hub Detection
Top N documents by incoming reference count (in-degree). N is configurable, default 10.

### Broken Link Detection
Outgoing wiki links and local markdown links that cannot be resolved to any indexed document. Use RealmIndex::find_uri_by_stem for wiki links.

### Summary Stats
- Total document count
- Total link count (wiki + markdown internal)
- Orphan count
- Broken link count
- Most connected document (hub)

### Cluster Analysis (optional, enabled by include_clusters flag)
Weakly connected components in the link graph. Each cluster lists member URIs and cluster size. Skip if include_clusters=false (expensive for large workspaces).

## Implementation Steps

1. Write failing tests in markymark-mcp/tests/graph_analysis_tests.rs
2. Add GraphAnalysis variant to CoreOperation enum
3. Add GraphAnalysis result types (struct GraphAnalysisResult) and CoreOperationResult::GraphAnalysis variant
4. Implement graph_analysis_computation function in runtime_engine.rs
5. Wire up execute() match arm for CoreOperation::GraphAnalysis
6. Add DTO types (GraphAnalysisRequest) to dto.rs
7. Implement graph_analysis_tool method in lib.rs, register in tool_router
8. Run tests GREEN
9. cargo clippy clean
10. cargo nextest passing

## Success Criteria
- [ ] graph-analysis tool appears in list_tools output
- [ ] Returns orphan list for workspace with known isolated documents
- [ ] Returns hub list sorted by incoming reference count
- [ ] Returns broken_links list for wiki links pointing to non-existent pages
- [ ] Returns summary stats (doc count, link count, orphan count, broken count)
- [ ] include_clusters=false is the default (performance)
- [ ] include_clusters=true returns connected components
- [ ] All new tests pass
- [ ] No regressions in existing tests
- [ ] cargo clippy clean
