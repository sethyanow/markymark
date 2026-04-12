# Toolchain Development

## Why Toolchains?

Toolchains decouple rules from platform-specific tools:
- Rules declare they need "a compiler" (toolchain type)
- Toolchains provide specific compilers for each platform
- Bazel automatically selects the right toolchain

## Complete Toolchain Setup

### 1. Define Toolchain Type

```python
# //my_lang:BUILD
toolchain_type(
    name = "toolchain_type",
    visibility = ["//visibility:public"],
)
```

### 2. Create Toolchain Rule

```python
# //my_lang:toolchain.bzl

MyLangToolchainInfo = provider(fields = {
    "compiler": "Compiler executable",
    "stdlib": "Standard library files",
    "compile_flags": "Default compilation flags",
})

def _my_lang_toolchain_impl(ctx):
    return [
        platform_common.ToolchainInfo(
            my_lang_info = MyLangToolchainInfo(
                compiler = ctx.executable.compiler,
                stdlib = ctx.files.stdlib,
                compile_flags = ctx.attr.compile_flags,
            ),
        ),
    ]

my_lang_toolchain = rule(
    implementation = _my_lang_toolchain_impl,
    attrs = {
        "compiler": attr.label(
            mandatory = True,
            executable = True,
            cfg = "exec",
        ),
        "stdlib": attr.label_list(
            allow_files = True,
        ),
        "compile_flags": attr.string_list(),
    },
    provides = [platform_common.ToolchainInfo],
)
```

### 3. Define Toolchain Instances

```python
# //my_lang/toolchains:BUILD

load("//my_lang:toolchain.bzl", "my_lang_toolchain")

my_lang_toolchain(
    name = "linux_x86_64_impl",
    compiler = "//my_lang/compilers:linux_x86_64",
    stdlib = ["//my_lang/stdlib:linux_x86_64"],
    compile_flags = ["-O2"],
)

toolchain(
    name = "linux_x86_64",
    toolchain = ":linux_x86_64_impl",
    toolchain_type = "//my_lang:toolchain_type",
    exec_compatible_with = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
    target_compatible_with = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
)

my_lang_toolchain(
    name = "macos_arm64_impl",
    compiler = "//my_lang/compilers:macos_arm64",
    stdlib = ["//my_lang/stdlib:macos_arm64"],
    compile_flags = ["-O2"],
)

toolchain(
    name = "macos_arm64",
    toolchain = ":macos_arm64_impl",
    toolchain_type = "//my_lang:toolchain_type",
    exec_compatible_with = [
        "@platforms//os:macos",
        "@platforms//cpu:arm64",
    ],
    target_compatible_with = [
        "@platforms//os:macos",
        "@platforms//cpu:arm64",
    ],
)
```

### 4. Register Toolchains

```python
# MODULE.bazel
register_toolchains(
    "//my_lang/toolchains:linux_x86_64",
    "//my_lang/toolchains:macos_arm64",
)
```

### 5. Use Toolchain in Rules

```python
# //my_lang:rules.bzl

def _my_lang_binary_impl(ctx):
    toolchain = ctx.toolchains["//my_lang:toolchain_type"]
    tc = toolchain.my_lang_info

    output = ctx.actions.declare_file(ctx.label.name)

    args = ctx.actions.args()
    args.add_all(tc.compile_flags)
    args.add("-o", output)
    args.add_all(ctx.files.srcs)

    ctx.actions.run(
        mnemonic = "MyLangCompile",
        executable = tc.compiler,
        arguments = [args],
        inputs = depset(ctx.files.srcs, transitive = [depset(tc.stdlib)]),
        outputs = [output],
    )

    return [DefaultInfo(
        files = depset([output]),
        executable = output,
    )]

my_lang_binary = rule(
    implementation = _my_lang_binary_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = [".ml"]),
    },
    toolchains = ["//my_lang:toolchain_type"],
    executable = True,
)
```

## Cross-Compilation

Toolchains enable cross-compilation by separating:
- **Execution platform**: Where build actions run
- **Target platform**: What platform the output runs on

```python
toolchain(
    name = "cross_linux_arm",
    toolchain = ":cross_compiler_impl",
    toolchain_type = "//my_lang:toolchain_type",
    # Build actions run on Linux x86_64
    exec_compatible_with = [
        "@platforms//os:linux",
        "@platforms//cpu:x86_64",
    ],
    # Output runs on Linux ARM
    target_compatible_with = [
        "@platforms//os:linux",
        "@platforms//cpu:arm64",
    ],
)
```

## Optional Toolchains

```python
my_rule = rule(
    implementation = _impl,
    toolchains = [
        config_common.toolchain_type("//my_lang:toolchain_type", mandatory = False),
    ],
)

def _impl(ctx):
    tc = ctx.toolchains["//my_lang:toolchain_type"]
    if tc:
        # Use toolchain
        ...
    else:
        # Fallback behavior
        ...
```

## Toolchain with Dependencies

Toolchains can have their own dependencies:

```python
def _my_toolchain_impl(ctx):
    return [platform_common.ToolchainInfo(
        compiler = ctx.executable.compiler,
        # cfg = "target" means these are for the output platform
        runtime_libs = ctx.attr.runtime_libs,
        # cfg = "exec" means these are for the build platform
        build_tools = ctx.files.build_tools,
    )]

my_lang_toolchain = rule(
    implementation = _my_toolchain_impl,
    attrs = {
        "compiler": attr.label(executable = True, cfg = "exec"),
        "runtime_libs": attr.label_list(cfg = "target"),
        "build_tools": attr.label_list(cfg = "exec"),
    },
)
```

## Debugging Toolchains

```bash
# See toolchain resolution
bazel build //my:target --toolchain_resolution_debug='.*'

# See which toolchain was selected
bazel cquery 'deps(//my:target, 1)' --output=starlark \
  --starlark:expr='providers(target).keys()'
```
