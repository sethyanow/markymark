# Starlark API Quick Reference

## ctx Object

### Attributes Access

| Property | Returns | Description |
|----------|---------|-------------|
| `ctx.attr.name` | Target value | Attribute values from rule call |
| `ctx.files.name` | `[File]` | Files from label/label_list attr |
| `ctx.file.name` | `File` | Single file (when `allow_single_file=True`) |
| `ctx.executable.name` | `File` | Executable file (when `executable=True`) |
| `ctx.outputs.name` | `File` | Predeclared outputs |

### Context Properties

| Property | Returns | Description |
|----------|---------|-------------|
| `ctx.label` | `Label` | Target's label |
| `ctx.label.name` | `str` | Target name |
| `ctx.label.package` | `str` | Package path |
| `ctx.workspace_name` | `str` | Workspace name |
| `ctx.bin_dir` | `root` | bazel-bin root |
| `ctx.genfiles_dir` | `root` | Generated files root |
| `ctx.toolchains["//type"]` | `ToolchainInfo` | Resolved toolchain |
| `ctx.fragments.cpp` | `fragment` | Configuration fragment |

### Actions

```python
# Declare output file
output = ctx.actions.declare_file(name)
output = ctx.actions.declare_file(name, sibling=other_file)

# Declare directory
dir = ctx.actions.declare_directory(name)

# Run executable
ctx.actions.run(
    mnemonic = "Name",           # Short name for progress
    executable = file,           # Tool to run
    arguments = [str],           # Command arguments
    inputs = depset/list,        # Input files
    outputs = [File],            # Output files
    env = {str: str},            # Environment variables
    execution_requirements = {}, # Execution constraints
    use_default_shell_env = False,
)

# Run shell command
ctx.actions.run_shell(
    mnemonic = "Name",
    command = "shell command",
    arguments = [str],           # Passed as $1, $2, etc.
    inputs = depset/list,
    outputs = [File],
    env = {},
)

# Write file
ctx.actions.write(
    output = File,
    content = str,
    is_executable = False,
)

# Expand template
ctx.actions.expand_template(
    template = File,
    output = File,
    substitutions = {"%key%": "value"},
)

# Build args efficiently
args = ctx.actions.args()
args.add("-flag")
args.add("-o", output)
args.add_all(files)
args.add_joined("-I", dirs, join_with=":")
```

### Runfiles

```python
runfiles = ctx.runfiles(
    files = [File],              # Explicit files
    transitive_files = depset,   # Additional files
    symlinks = {path: File},     # Symlinks in runfiles
)

# Merge from dependencies
runfiles = runfiles.merge(other_runfiles)
runfiles = runfiles.merge_all([rf1, rf2])
```

## Attribute Types

```python
attr.label(
    default = Label("//pkg:target"),
    allow_files = [".ext"],      # Or True for any
    allow_single_file = True,    # ctx.file instead of ctx.files
    executable = True,           # Must be executable
    cfg = "exec",                # "exec", "target", or transition
    providers = [InfoProvider],  # Required providers
    mandatory = True,
)

attr.label_list(
    allow_files = [".go"],
    providers = [GoInfo],
    cfg = "target",
)

attr.string(default = "", values = ["a", "b"])
attr.string_list(default = [])
attr.int(default = 0, values = [1, 2, 3])
attr.bool(default = False)
attr.string_dict(default = {})

attr.output()                    # Single output
attr.output_list()               # Multiple outputs
```

## File Object

| Property | Type | Description |
|----------|------|-------------|
| `file.path` | `str` | Execution-relative path |
| `file.short_path` | `str` | Runfiles-relative path |
| `file.basename` | `str` | File name without directory |
| `file.dirname` | `str` | Directory path |
| `file.extension` | `str` | File extension |
| `file.root` | `root` | Output root (bin/genfiles) |
| `file.is_source` | `bool` | True if source file |

## Label Object

| Property | Type | Description |
|----------|------|-------------|
| `label.name` | `str` | Target name |
| `label.package` | `str` | Package path |
| `label.workspace_name` | `str` | Repository name |
| `label.workspace_root` | `str` | Repository path |

## Depset

```python
# Create
d = depset(direct=[a, b], transitive=[d1, d2], order="default")

# Orders: "default", "postorder", "preorder", "topological"

# Convert to list (expensive - do in binary only)
items = d.to_list()

# Check membership (also expensive)
has_item = item in d.to_list()
```

## Provider

```python
# Define
MyInfo = provider(
    doc = "Description",
    fields = {
        "field1": "Description of field1",
        "field2": "Description of field2",
    },
)

# Create instance
info = MyInfo(field1 = value1, field2 = value2)

# Access from target
target[MyInfo].field1

# Check if target has provider
MyInfo in target
```

## Rule Definition

```python
my_rule = rule(
    implementation = _impl,
    attrs = {...},
    outputs = {"out": "%{name}.out"},  # Predeclared
    executable = True,           # For binaries
    test = True,                 # For tests (name must end in _test)
    toolchains = ["//type"],     # Required toolchains
    fragments = ["cpp"],         # Config fragments
    provides = [MyInfo],         # Guaranteed providers
    cfg = transition,            # Configuration transition
    doc = "Rule documentation",
)
```

## Built-in Providers

| Provider | Fields | Use |
|----------|--------|-----|
| `DefaultInfo` | `files`, `runfiles`, `executable` | Default outputs |
| `OutputGroupInfo` | `<group_name>` | Named output groups |
| `RunEnvironmentInfo` | `environment`, `inherited_environment` | Test env |
| `InstrumentedFilesInfo` | `...` | Coverage support |
| `ToolchainInfo` | Custom fields | Toolchain data |
