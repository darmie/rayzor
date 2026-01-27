# Dockerfile for testing intermittent thread crashes on amd64 Linux
#
# Mirrors the CI environment (Rust stable + LLVM 18 + all-backends).
# Uses Debian Bookworm (stable) for LLVM 18 apt repository support.
#
# Build:
#   docker build --platform linux/amd64 -t rayzor-thread-test -f ci/thread-test/Dockerfile .
#
# Run stress test (default: 20 iterations):
#   docker run --platform linux/amd64 rayzor-thread-test
#
# Run with custom iterations:
#   docker run --platform linux/amd64 rayzor-thread-test ./ci/thread-test/stress-test.sh 50
#
# Interactive shell for debugging:
#   docker run --platform linux/amd64 -it rayzor-thread-test bash

FROM rust:bookworm

# Install LLVM 18 on Debian Bookworm
RUN apt-get update && apt-get install -y --no-install-recommends \
    wget \
    gnupg \
    lsb-release \
    git \
    ca-certificates \
    && wget -qO /usr/share/keyrings/llvm-snapshot.gpg.key https://apt.llvm.org/llvm-snapshot.gpg.key \
    && gpg --dearmor < /usr/share/keyrings/llvm-snapshot.gpg.key > /usr/share/keyrings/llvm-snapshot.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/llvm-snapshot.gpg] http://apt.llvm.org/bookworm/ llvm-toolchain-bookworm-18 main" \
       > /etc/apt/sources.list.d/llvm-18.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends llvm-18 llvm-18-dev libpolly-18-dev \
    && rm -rf /var/lib/apt/lists/*

ENV LLVM_SYS_180_PREFIX=/usr/lib/llvm-18
ENV CARGO_TERM_COLOR=always
# Use system git for fetches (libgit2 OOMs on large wasmtime submodules under emulation)
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
# Limit parallelism to avoid OOM under emulation
ENV CARGO_BUILD_JOBS=2

WORKDIR /rayzor

# Copy full source (manifests + code together to simplify build)
COPY . .

# Build the two thread test examples in release mode with all backends
RUN cargo build --release --package compiler --features all-backends \
    --example test_sys_thread \
    --example test_rayzor_stdlib_e2e

# Make stress test script executable
RUN chmod +x ci/thread-test/stress-test.sh

CMD ["./ci/thread-test/stress-test.sh"]
