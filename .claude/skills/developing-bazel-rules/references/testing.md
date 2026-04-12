# Testing Bazel Rules

## Analysis Tests

Test rule behavior without executing actions using `analysistest` from bazel_skylib.

### Basic Analysis Test

```python
# my_rules_test.bzl
load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//my_lang:rules.bzl", "my_library")

def _provider_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)

    # Check provider exists
    asserts.true(env, MyInfo in target, "Expected MyInfo provider")

    # Check provider values
    info = target[MyInfo]
    asserts.equals(env, "expected_value", info.some_field)

    # Check file count
    asserts.equals(env, 2, len(info.sources.to_list()))

    return analysistest.end(env)

provider_test = analysistest.make(_provider_test_impl)

def _test_my_library_provider():
    my_library(
        name = "target_under_test",
        srcs = ["a.ml", "b.ml"],
        tags = ["manual"],
    )
    provider_test(
        name = "my_library_provider_test",
        target_under_test = ":target_under_test",
    )

def my_library_test_suite(name):
    _test_my_library_provider()

    native.test_suite(
        name = name,
        tests = [":my_library_provider_test"],
    )
```

### Testing Actions

```python
def _action_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    actions = analysistest.target_actions(env)

    # Find specific action by mnemonic
    compile_actions = [a for a in actions if a.mnemonic == "MyCompile"]
    asserts.equals(env, 1, len(compile_actions))

    action = compile_actions[0]

    # Check arguments
    asserts.true(env, "-O2" in action.argv)

    # Check inputs/outputs
    input_paths = [f.path for f in action.inputs.to_list()]
    asserts.true(env, any("source.ml" in p for p in input_paths))

    return analysistest.end(env)

action_test = analysistest.make(_action_test_impl)
```

### Testing Failures

```python
def _failure_test_impl(ctx):
    env = analysistest.begin(ctx)
    # Test expects failure, nothing to assert if it reaches here
    asserts.expect_failure(env, "missing required attribute")
    return analysistest.end(env)

failure_test = analysistest.make(
    _failure_test_impl,
    expect_failure = True,
)

def _test_missing_srcs_fails():
    my_library(
        name = "missing_srcs_target",
        # srcs intentionally missing
        tags = ["manual"],
    )
    failure_test(
        name = "missing_srcs_fails_test",
        target_under_test = ":missing_srcs_target",
    )
```

### Testing with Configuration

```python
def _config_test_impl(ctx):
    env = analysistest.begin(ctx)
    # Access configuration
    config = ctx.configuration
    # Test config-dependent behavior
    return analysistest.end(env)

config_test = analysistest.make(
    _config_test_impl,
    config_settings = {
        "//my_lang:some_flag": "value",
    },
)
```

## Integration Tests

Test actual build output with `sh_test` or `py_test`:

```python
# BUILD
load("//my_lang:rules.bzl", "my_binary")

my_binary(
    name = "hello",
    srcs = ["hello.ml"],
)

sh_test(
    name = "hello_test",
    srcs = ["hello_test.sh"],
    data = [":hello"],
)
```

```bash
# hello_test.sh
#!/bin/bash
OUTPUT=$(./hello)
if [[ "$OUTPUT" != "Hello, World!" ]]; then
    echo "Expected 'Hello, World!' but got '$OUTPUT'"
    exit 1
fi
```

## Testing Output File Contents

```python
# file_content_test.bzl
def _file_content_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)

    # Get output files
    files = target[DefaultInfo].files.to_list()
    asserts.equals(env, 1, len(files))

    # Check file extension
    asserts.true(env, files[0].path.endswith(".out"))

    return analysistest.end(env)
```

## Testing Toolchains

```python
def _toolchain_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    actions = analysistest.target_actions(env)

    # Verify toolchain was used
    compile_action = [a for a in actions if a.mnemonic == "MyCompile"][0]

    # Check toolchain executable was used
    asserts.true(
        env,
        compile_action.executable.path.endswith("my_compiler"),
        "Expected toolchain compiler",
    )

    return analysistest.end(env)
```

## Testing Aspects

```python
def _aspect_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)

    # Check aspect provider
    asserts.true(env, MyAspectInfo in target)

    return analysistest.end(env)

aspect_test = analysistest.make(
    _aspect_test_impl,
    extra_target_under_test_aspects = [my_aspect],
)
```

## Test Suite Organization

```python
# tests/BUILD
load(":my_library_tests.bzl", "my_library_test_suite")
load(":my_binary_tests.bzl", "my_binary_test_suite")

my_library_test_suite(name = "my_library_tests")
my_binary_test_suite(name = "my_binary_tests")

test_suite(
    name = "all_tests",
    tests = [
        ":my_library_tests",
        ":my_binary_tests",
    ],
)
```

## Debugging Test Failures

```bash
# Run with verbose output
bazel test //tests:my_test --test_output=all

# Run analysis tests with debug
bazel test //tests:my_test --test_output=errors --verbose_failures

# Check what targets are created
bazel query 'deps(//tests:my_test)'
```

## Best Practices

1. **Test providers** - Verify all expected providers are returned
2. **Test actions** - Check mnemonic, inputs, outputs, arguments
3. **Test failures** - Ensure invalid inputs fail appropriately
4. **Test configurations** - Cover different build settings
5. **Test output groups** - Verify validation outputs
6. **Use `tags = ["manual"]`** - Prevent test targets from normal builds
