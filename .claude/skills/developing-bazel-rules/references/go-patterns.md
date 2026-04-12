# Patterns from rules_go

These patterns are extracted from [rules_go](https://github.com/bazel-contrib/rules_go), a production-quality Bazel ruleset.

## Go Context Pattern

Create a unified context object that encapsulates toolchain access and common operations:

```python
# context.bzl
def go_context(ctx, include_deprecated_properties = True):
    """Create a Go build context from a rule context."""
    toolchain = ctx.toolchains["@rules_go//go:toolchain"]

    return struct(
        # SDK and toolchain
        go = toolchain.sdk.go,
        sdk = toolchain.sdk,
        stdlib = ctx.attr._stdlib[GoStdLib],

        # Configuration
        mode = struct(
            goos = toolchain.default_goos,
            goarch = toolchain.default_goarch,
            race = ctx.attr.race == "on",
            pure = ctx.attr.pure == "on",
        ),

        # Convenience methods
        actions = ctx.actions,
        declare_file = lambda name: ctx.actions.declare_file(name),
        label = ctx.label,

        # Action emitters
        archive = toolchain.actions.archive,
        link = toolchain.actions.link,
    )
```

Usage in implementation:

```python
def _go_library_impl(ctx):
    go = go_context(ctx)

    # Use context for all operations
    archive = go.archive(go, sources)

    return [GoInfo(...), archive]
```

## Layered Provider Architecture

Separate providers for different purposes:

```python
# 1. Source-level info (before compilation)
GoInfo = provider(fields = {
    "srcs": "Source files",
    "deps": "Direct dependencies",
    "importpath": "Import path",
})

# 2. Compiled archive (single target)
GoArchive = provider(fields = {
    "source": "GoInfo this was built from",
    "file": "The compiled .a file",
    "export_file": "Export data for incremental compilation",
    "data": "GoArchiveData for transitive use",
})

# 3. Lightweight transitive data
GoArchiveData = provider(fields = {
    "file": "Archive file",
    "importpath": "Import path",
    "importmap": "Real path (may differ for vendor)",
})
```

## Embed Pattern

Allow composing sources from multiple targets:

```python
"embed": attr.label_list(
    providers = [GoInfo],
    doc = "Libraries whose sources compile with this package",
),

def _impl(ctx):
    srcs = list(ctx.files.srcs)
    deps = list(ctx.attr.deps)

    for embed in ctx.attr.embed:
        embed_info = embed[GoInfo]
        srcs.extend(embed_info.srcs.to_list())
        deps.extend(embed_info.deps)
```

## Mode-Based Configuration

Use string attributes with controlled values for build modes:

```python
"pure": attr.string(
    default = "auto",
    values = ["on", "off", "auto"],
    doc = "Disable cgo",
),
"race": attr.string(
    default = "auto",
    values = ["on", "off", "auto"],
    doc = "Enable race detector",
),
"linkmode": attr.string(
    default = "auto",
    values = ["auto", "normal", "pie", "plugin", "c-shared", "c-archive"],
),
```

Resolve "auto" based on configuration:

```python
def resolve_mode(ctx, toolchain):
    pure = ctx.attr.pure
    if pure == "auto":
        pure = "on" if toolchain.cross_compile else "off"

    return struct(
        pure = pure == "on",
        race = ctx.attr.race == "on",
        # ...
    )
```

## Action Emitter Pattern

Separate action creation into reusable functions:

```python
# actions/archive.bzl
def emit_archive(go, source):
    """Compile Go sources to an archive."""
    out = go.declare_file(go, ext = ".a")

    args = go.actions.args()
    args.add("-o", out)
    args.add("-importpath", source.importpath)
    args.add_all(source.srcs)

    go.actions.run(
        mnemonic = "GoCompile",
        executable = go.toolchain._builder,
        arguments = [args],
        inputs = depset(source.srcs, transitive = [...]),
        outputs = [out],
    )

    return GoArchive(
        source = source,
        file = out,
        # ...
    )
```

## Reset Target Pattern

Create targets that reset configuration for tool binaries:

```python
# Avoid tools being built with race detection, coverage, etc.
go_reset_target = rule(
    implementation = _go_reset_target_impl,
    attrs = {
        "dep": attr.label(mandatory = True),
        "_allowlist_function_transition": attr.label(
            default = "@bazel_tools//tools/allowlists/function_transition_allowlist",
        ),
    },
    cfg = _go_reset_transition,
)

def _go_reset_transition_impl(settings, attr):
    return {
        "//go/config:race": False,
        "//go/config:msan": False,
        "//go/config:pure": True,
    }
```

## Validation Output Pattern

Use validation output group for checks that don't block compilation:

```python
def _impl(ctx):
    # Main compilation
    archive = compile(...)

    # Validation (linting, etc.)
    validation_output = None
    if ctx.attr._nogo:
        validation_output = run_nogo(ctx, archive)

    return [
        DefaultInfo(files = depset([archive.file])),
        OutputGroupInfo(
            compilation_outputs = [archive.file],
            _validation = [validation_output] if validation_output else [],
        ),
    ]
```

## Cross-Compilation Pattern

Use goos/goarch attributes with transitions:

```python
"goos": attr.string(
    default = "auto",
    doc = "Target OS. auto = use platform",
),
"goarch": attr.string(
    default = "auto",
    doc = "Target arch. auto = use platform",
),

def _go_cross_transition_impl(settings, attr):
    goos = attr.goos
    goarch = attr.goarch

    if goos == "auto":
        goos = settings["//go/toolchain:goos"]
    if goarch == "auto":
        goarch = settings["//go/toolchain:goarch"]

    return {
        "//go/toolchain:goos": goos,
        "//go/toolchain:goarch": goarch,
    }
```

## Runfiles Pattern

Proper runfiles merging for binaries:

```python
def _go_binary_impl(ctx):
    runfiles = ctx.runfiles(files = ctx.files.data)

    # Merge from all dependency types
    for runfiles_attr in (ctx.attr.srcs, ctx.attr.deps, ctx.attr.data):
        for target in runfiles_attr:
            runfiles = runfiles.merge(target[DefaultInfo].default_runfiles)

    return [DefaultInfo(
        executable = executable,
        runfiles = runfiles,
    )]
```

## Gazelle Integration

Design rules to work with Gazelle for automatic BUILD file generation:

```python
# Standard attribute names Gazelle expects
"srcs": attr.label_list(allow_files = [".go"]),
"deps": attr.label_list(providers = [GoInfo]),
"importpath": attr.string(),
"embed": attr.label_list(providers = [GoInfo]),
```

Gazelle directives in BUILD files:

```python
# gazelle:prefix github.com/example/repo
# gazelle:go_naming_convention go_default_library
```
