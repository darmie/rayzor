#!/usr/bin/env bash
#
# Stress test for intermittent benchmark crashes on amd64 Linux.
#
# Runs benchmark_runner with mandelbrot repeatedly to reproduce
# crashes that occur intermittently in CI.
#
# Usage:
#   ./ci/bench-test/stress-test.sh [ITERATIONS] [--bench NAME] [--stop-on-fail]
#
# Examples:
#   ./ci/bench-test/stress-test.sh              # 20 iterations, mandelbrot
#   ./ci/bench-test/stress-test.sh 50           # 50 iterations
#   ./ci/bench-test/stress-test.sh 30 --bench mandelbrot_simple
#   ./ci/bench-test/stress-test.sh 10 --stop-on-fail

set -uo pipefail

# --- Configuration ---

ITERATIONS="${1:-20}"
BENCH_NAME="mandelbrot"
STOP_ON_FAIL=0
LOG_DIR="ci/bench-test/logs"
BINARY="target/release/examples/benchmark_runner"

# Parse flags from remaining args
shift 2>/dev/null || true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --bench)
            BENCH_NAME="$2"
            shift 2
            ;;
        --stop-on-fail)
            STOP_ON_FAIL=1
            shift
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
SUMMARY_LOG="$LOG_DIR/summary_${TIMESTAMP}.log"

pass_count=0
fail_count=0
signals=()

# --- Helpers ---

describe_exit() {
    local code=$1
    if [[ $code -eq 0 ]]; then
        echo "OK"
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

# --- Banner ---

echo "========================================================================"
echo "  Rayzor Benchmark Stress Test"
echo "========================================================================"
echo ""
echo "  Platform:    $(uname -s) $(uname -m)"
echo "  Kernel:      $(uname -r)"
echo "  Iterations:  $ITERATIONS"
echo "  Benchmark:   $BENCH_NAME"
echo "  Stop on fail: $( [[ $STOP_ON_FAIL -eq 1 ]] && echo "yes" || echo "no" )"
echo "  Log dir:     $LOG_DIR"
echo "  Timestamp:   $TIMESTAMP"
echo ""
echo "========================================================================"

{
    echo "Rayzor Benchmark Stress Test"
    echo "Platform: $(uname -s) $(uname -m) | Kernel: $(uname -r)"
    echo "Iterations: $ITERATIONS | Benchmark: $BENCH_NAME | Stop on fail: $STOP_ON_FAIL"
    echo "Started: $(date)"
    echo ""
} > "$SUMMARY_LOG"

# --- Test loop ---

for ((i = 1; i <= ITERATIONS; i++)); do
    printf "  [RUN ] %s (#%d/%d)... " "$BENCH_NAME" "$i" "$ITERATIONS"
    logfile="$LOG_DIR/${BENCH_NAME}_${TIMESTAMP}_run${i}.log"

    # Run with signal handler preloaded and timeout
    # RAYZOR_TARGET can be set to isolate a specific backend (cranelift, interpreter, tiered)
    exit_code=0
    RAYZOR_TARGET=${RAYZOR_TARGET:-} LD_PRELOAD=/usr/lib/libseghandler.so timeout 300 \
        "$BINARY" "$BENCH_NAME" > "$logfile" 2>&1 || exit_code=$?

    if [[ $exit_code -eq 0 ]]; then
        echo "PASS"
        pass_count=$((pass_count + 1))
    else
        desc=$(describe_exit "$exit_code")
        echo "FAIL ($desc)"
        fail_count=$((fail_count + 1))
        signals+=("run$i:$desc")
        echo "         Log: $logfile"

        echo "  [FAIL] run $i: $desc" >> "$SUMMARY_LOG"

        # Print last 30 lines of log for quick diagnosis
        echo "         --- last 30 lines ---"
        tail -30 "$logfile" 2>/dev/null | sed 's/^/         /'
        echo "         --- end ---"

        if [[ $STOP_ON_FAIL -eq 1 ]]; then
            echo ""
            echo "Stopping on first failure (--stop-on-fail)"
            break
        fi
    fi
done

# --- Summary ---

echo ""
echo "========================================================================"
echo "  STRESS TEST SUMMARY"
echo "========================================================================"
echo ""

total=$((pass_count + fail_count))

echo "  Benchmark: $BENCH_NAME"
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
