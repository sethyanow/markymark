FROM ghcr.io/cross-rs/aarch64-unknown-linux-gnu:main

ARG ZIG_VERSION=0.15.2
ARG ZIG_ARCH=x86_64-linux

RUN apt-get update && apt-get install -y --no-install-recommends xz-utils \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL "https://ziglang.org/download/${ZIG_VERSION}/zig-${ZIG_ARCH}-${ZIG_VERSION}.tar.xz" \
    | tar -xJ -C /usr/local \
    && ln -s "/usr/local/zig-${ZIG_ARCH}-${ZIG_VERSION}/zig" /usr/local/bin/zig

ENV ZIG_GLOBAL_CACHE_DIR=/tmp/zig-cache

RUN zig version
