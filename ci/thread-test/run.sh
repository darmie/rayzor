#!/usr/bin/env bash
#
# Build and run the amd64 Linux Docker container for thread stress testing.
#
# Usage:
#   ./ci/thread-test/run.sh                    # Build + run 20 iterations
#   ./ci/thread-test/run.sh 50                 # Build + run 50 iterations
#   ./ci/thread-test/run.sh 100 --stop-on-fail # Stop on first failure
#   ./ci/thread-test/run.sh --shell            # Build + open interactive shell
#   ./ci/thread-test/run.sh --rebuild          # Force rebuild, then run
#
# The container runs on linux/amd64 (emulated via QEMU/Rosetta if on ARM Mac).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
IMAGE_NAME="rayzor-thread-test"
PLATFORM="linux/amd64"

cd "$PROJECT_ROOT"

# Parse our flags (pass the rest through to stress-test.sh)
SHELL_MODE=0
FORCE_REBUILD=0
PASSTHROUGH_ARGS=()

for arg in "$@"; do
    case "$arg" in
        --shell)
            SHELL_MODE=1
            ;;
        --rebuild)
            FORCE_REBUILD=1
            ;;
        *)
            PASSTHROUGH_ARGS+=("$arg")
            ;;
    esac
done

# --- Build ---

echo "Building Docker image: $IMAGE_NAME (platform: $PLATFORM)"
echo ""

BUILD_ARGS=(
    --platform "$PLATFORM"
    -t "$IMAGE_NAME"
    -f ci/thread-test/Dockerfile
)

if [[ $FORCE_REBUILD -eq 1 ]]; then
    BUILD_ARGS+=(--no-cache)
fi

docker build "${BUILD_ARGS[@]}" .

echo ""
echo "Build complete."
echo ""

# --- Run ---

RUN_ARGS=(
    --platform "$PLATFORM"
    --rm
)

if [[ $SHELL_MODE -eq 1 ]]; then
    echo "Opening interactive shell in container..."
    echo "  Run tests manually with:"
    echo "    ./ci/thread-test/stress-test.sh 20"
    echo "    cargo run --release --package compiler --features all-backends --example test_sys_thread"
    echo ""
    docker run "${RUN_ARGS[@]}" -it "$IMAGE_NAME" bash
else
    if [[ ${#PASSTHROUGH_ARGS[@]} -gt 0 ]]; then
        docker run "${RUN_ARGS[@]}" "$IMAGE_NAME" ./ci/thread-test/stress-test.sh "${PASSTHROUGH_ARGS[@]}"
    else
        docker run "${RUN_ARGS[@]}" "$IMAGE_NAME"
    fi
fi
