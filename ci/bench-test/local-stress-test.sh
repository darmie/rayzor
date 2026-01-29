#!/usr/bin/env bash
#
# Local stress test for Mac (and Linux without Docker).
#
# Runs benchmark_runner repeatedly to catch intermittent crashes
# across all backend targets (cranelift, tiered, precompiled-tiered).
#
# Usage:
#   ./ci/bench-test/local-stress-test.sh [ITERATIONS] [--bench NAME] [--stop-on-fail]
#
# Examples:
#   ./ci/bench-test/local-stress-test.sh              # 10 iterations, mandelbrot + nbody
#   ./ci/bench-test/local-stress-test.sh 20            # 20 iterations
#   ./ci/bench-test/local-stress-test.sh 5 --bench nbody
#   ./ci/bench-test/local-stress-test.sh 10 --stop-on-fail

set -uo pipefail

# --- Configuration ---

ITERATIONS="${1:-10}"
BENCHMARKS="mandelbrot nbody"
STOP_ON_FAIL=0
TIMEOUT=120
LOG_DIR="ci/bench-test/logs"
BINARY="target/release/examples/benchmark_runner"

# Parse flags from remaining args
shift 2>/dev/null || true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bench)
            BENCHMARKS="$2"
            shift 2
            ;;
        --stop-on-fail)
            STOP_ON_FAIL=1
            shift
            ;;
        --timeout)
            TIMEOUT="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1"
            exit 1
            ;;
    esac
done

# --- Setup ---

mkdir -p "$LOG_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SUMMARY_LOG="$LOG_DIR/local_summary_${TIMESTAMP}.log"

pass_count=0
fail_count=0
signals=()

# --- Helpers ---

describe_exit() {
    local code=$1
    if [[ $code -eq 0 ]]; then
        echo "OK"
    elif [[ $code -eq 124 ]]; then
        echo "TIMEOUT"
    elif [[ $code -gt 128 ]]; then
        local sig=$((code - 128))
        case $sig in
            6)  echo "SIGABRT (signal 6)" ;;
            9)  echo "SIGKILL (signal 9)" ;;
            11) echo "SIGSEGV (signal 11)" ;;
            15) echo "SIGTERM (signal 15)" ;;
            *)  echo "signal $sig" ;;
        esac
    else
        echo "exit code $code"
    fi
}

# --- Check binary ---

if [[ ! -f "$BINARY" ]]; then
    echo "Binary not found: $BINARY"
    echo "Building release binary..."
    cargo build --release -p compiler --features llvm-backend --example benchmark_runner || exit 1
fi

# --- Banner ---

echo "========================================================================"
echo "  Rayzor Local Stress Test"
echo "========================================================================"
echo ""
echo "  Platform:    $(uname -s) $(uname -m)"
echo "  Kernel:      $(uname -r)"
echo "  Iterations:  $ITERATIONS"
echo "  Benchmarks:  $BENCHMARKS"
echo "  Timeout:     ${TIMEOUT}s per run"
echo "  Stop on fail: $( [[ $STOP_ON_FAIL -eq 1 ]] && echo "yes" || echo "no" )"
echo "  Log dir:     $LOG_DIR"
echo ""
echo "========================================================================"

{
    echo "Rayzor Local Stress Test"
    echo "Platform: $(uname -s) $(uname -m) | Kernel: $(uname -r)"
    echo "Iterations: $ITERATIONS | Benchmarks: $BENCHMARKS"
    echo "Started: $(date)"
    echo ""
} > "$SUMMARY_LOG"

# --- Test loop ---

stopped=0
for bench in $BENCHMARKS; do
    if [[ $stopped -eq 1 ]]; then break; fi

    echo ""
    echo "  --- $bench ---"

    for ((i = 1; i <= ITERATIONS; i++)); do
        printf "  [RUN ] %s (#%d/%d)... " "$bench" "$i" "$ITERATIONS"
        logfile="$LOG_DIR/${bench}_local_${TIMESTAMP}_run${i}.log"

        exit_code=0
        timeout "$TIMEOUT" "$BINARY" "$bench" > "$logfile" 2>&1 || exit_code=$?

        if [[ $exit_code -eq 0 ]]; then
            echo "PASS"
            pass_count=$((pass_count + 1))
        else
            desc=$(describe_exit "$exit_code")
            echo "FAIL ($desc)"
            fail_count=$((fail_count + 1))
            signals+=("${bench}:run$i:$desc")
            echo "         Log: $logfile"

            echo "  [FAIL] $bench run $i: $desc" >> "$SUMMARY_LOG"

            echo "         --- last 20 lines ---"
            tail -20 "$logfile" 2>/dev/null | sed 's/^/         /'
            echo "         --- end ---"

            if [[ $STOP_ON_FAIL -eq 1 ]]; then
                echo ""
                echo "Stopping on first failure (--stop-on-fail)"
                stopped=1
                break
            fi
        fi
    done
done

# --- Summary ---

echo ""
echo "========================================================================"
echo "  LOCAL STRESS TEST SUMMARY"
echo "========================================================================"
echo ""

total=$((pass_count + fail_count))

echo "  Benchmarks: $BENCHMARKS"
echo "    Passed: $pass_count / $total"
echo "    Failed: $fail_count / $total"
if [[ ${#signals[@]} -gt 0 ]]; then
    echo "    Failures:"
    for sig in "${signals[@]}"; do
        echo "      - $sig"
    done
fi
echo ""

if [[ $fail_count -gt 0 ]]; then
    fail_rate=$((fail_count * 100 / total))
    echo "  Failure rate: ${fail_rate}% ($fail_count / $total)"
fi
echo ""
echo "  Full logs: $LOG_DIR/"
echo "  Summary:   $SUMMARY_LOG"
echo "========================================================================"

# Append final summary to log
{
    echo ""
    echo "Finished: $(date)"
    echo "Total: $pass_count / $total passed, $fail_count failed"
    if [[ $fail_count -gt 0 ]]; then
        echo "Failure rate: $((fail_count * 100 / total))%"
    fi
} >> "$SUMMARY_LOG"

# Exit with failure if any test failed
if [[ $fail_count -gt 0 ]]; then
    exit 1
fi
