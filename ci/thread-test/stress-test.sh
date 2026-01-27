#!/usr/bin/env bash
#
# Stress test for intermittent thread crashes on amd64 Linux.
#
# Runs test_sys_thread and test_rayzor_stdlib_e2e repeatedly to reproduce
# threading issues that occur intermittently in CI.
#
# Usage:
#   ./ci/thread-test/stress-test.sh [ITERATIONS] [--test sys_thread|stdlib_e2e|both] [--stop-on-fail]
#
# Examples:
#   ./ci/thread-test/stress-test.sh              # 20 iterations, both tests
#   ./ci/thread-test/stress-test.sh 50           # 50 iterations, both tests
#   ./ci/thread-test/stress-test.sh 100 --test sys_thread --stop-on-fail
#
# Environment variables:
#   RAYZOR_STRESS_ITERATIONS  - Number of iterations (default: 20)
#   RAYZOR_STRESS_TEST        - Which test: sys_thread, stdlib_e2e, both (default: both)
#   RAYZOR_STOP_ON_FAIL       - Stop on first failure: 1 or 0 (default: 0)

set -euo pipefail

# --- Configuration ---

ITERATIONS="${1:-${RAYZOR_STRESS_ITERATIONS:-20}}"
TEST_FILTER="${RAYZOR_STRESS_TEST:-both}"
STOP_ON_FAIL="${RAYZOR_STOP_ON_FAIL:-0}"
LOG_DIR="ci/thread-test/logs"

# Parse flags from remaining args
shift 2>/dev/null || true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --test)
            TEST_FILTER="$2"
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

# --- Resolve binary paths ---

# Check for pre-built release binaries first, then fall back to cargo run
SYS_THREAD_BIN=""
STDLIB_E2E_BIN=""

if [[ -f "target/release/examples/test_sys_thread" ]]; then
    SYS_THREAD_BIN="target/release/examples/test_sys_thread"
elif command -v cargo &>/dev/null; then
    SYS_THREAD_BIN="cargo run --release --package compiler --features all-backends --example test_sys_thread --"
fi

if [[ -f "target/release/examples/test_rayzor_stdlib_e2e" ]]; then
    STDLIB_E2E_BIN="target/release/examples/test_rayzor_stdlib_e2e"
elif command -v cargo &>/dev/null; then
    STDLIB_E2E_BIN="cargo run --release --package compiler --features all-backends --example test_rayzor_stdlib_e2e --"
fi

# --- Setup ---

mkdir -p "$LOG_DIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
SUMMARY_LOG="$LOG_DIR/summary_${TIMESTAMP}.log"

# Counters
sys_thread_pass=0
sys_thread_fail=0
stdlib_e2e_pass=0
stdlib_e2e_fail=0
sys_thread_signals=()
stdlib_e2e_signals=()

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

run_test() {
    local name="$1"
    local cmd="$2"
    local iteration="$3"
    local logfile="$LOG_DIR/${name}_${TIMESTAMP}_run${iteration}.log"

    # Run with timeout (120s) and capture output + exit code
    local exit_code=0
    timeout 120 $cmd > "$logfile" 2>&1 || exit_code=$?

    echo "$exit_code"
}

# --- Banner ---

echo "========================================================================"
echo "  Rayzor Thread Stress Test"
echo "========================================================================"
echo ""
echo "  Platform:    $(uname -s) $(uname -m)"
echo "  Kernel:      $(uname -r)"
echo "  Iterations:  $ITERATIONS"
echo "  Tests:       $TEST_FILTER"
echo "  Stop on fail: $( [[ $STOP_ON_FAIL -eq 1 ]] && echo "yes" || echo "no" )"
echo "  Log dir:     $LOG_DIR"
echo "  Timestamp:   $TIMESTAMP"
echo ""
echo "========================================================================"

{
    echo "Rayzor Thread Stress Test"
    echo "Platform: $(uname -s) $(uname -m) | Kernel: $(uname -r)"
    echo "Iterations: $ITERATIONS | Tests: $TEST_FILTER | Stop on fail: $STOP_ON_FAIL"
    echo "Started: $(date)"
    echo ""
} > "$SUMMARY_LOG"

# --- Test loop ---

for ((i = 1; i <= ITERATIONS; i++)); do
    echo ""
    echo "--- Iteration $i / $ITERATIONS ---"

    # test_sys_thread
    if [[ "$TEST_FILTER" == "both" || "$TEST_FILTER" == "sys_thread" ]]; then
        if [[ -z "$SYS_THREAD_BIN" ]]; then
            echo "  [SKIP] test_sys_thread - binary not found"
        else
            printf "  [RUN ] test_sys_thread (#%d)... " "$i"
            exit_code=$(run_test "sys_thread" "$SYS_THREAD_BIN" "$i")

            if [[ $exit_code -eq 0 ]]; then
                echo "PASS"
                sys_thread_pass=$((sys_thread_pass + 1))
            else
                desc=$(describe_exit "$exit_code")
                echo "FAIL ($desc)"
                sys_thread_fail=$((sys_thread_fail + 1))
                sys_thread_signals+=("run$i:$desc")
                echo "         Log: $LOG_DIR/sys_thread_${TIMESTAMP}_run${i}.log"

                echo "  [FAIL] sys_thread run $i: $desc" >> "$SUMMARY_LOG"

                # Print last 20 lines of log for quick diagnosis
                echo "         --- last 20 lines ---"
                tail -20 "$LOG_DIR/sys_thread_${TIMESTAMP}_run${i}.log" 2>/dev/null | sed 's/^/         /'
                echo "         --- end ---"

                if [[ $STOP_ON_FAIL -eq 1 ]]; then
                    echo ""
                    echo "Stopping on first failure (--stop-on-fail)"
                    break 2
                fi
            fi
        fi
    fi

    # test_rayzor_stdlib_e2e
    if [[ "$TEST_FILTER" == "both" || "$TEST_FILTER" == "stdlib_e2e" ]]; then
        if [[ -z "$STDLIB_E2E_BIN" ]]; then
            echo "  [SKIP] test_rayzor_stdlib_e2e - binary not found"
        else
            printf "  [RUN ] test_rayzor_stdlib_e2e (#%d)... " "$i"
            exit_code=$(run_test "stdlib_e2e" "$STDLIB_E2E_BIN" "$i")

            if [[ $exit_code -eq 0 ]]; then
                echo "PASS"
                stdlib_e2e_pass=$((stdlib_e2e_pass + 1))
            else
                desc=$(describe_exit "$exit_code")
                echo "FAIL ($desc)"
                stdlib_e2e_fail=$((stdlib_e2e_fail + 1))
                stdlib_e2e_signals+=("run$i:$desc")
                echo "         Log: $LOG_DIR/stdlib_e2e_${TIMESTAMP}_run${i}.log"

                echo "  [FAIL] stdlib_e2e run $i: $desc" >> "$SUMMARY_LOG"

                echo "         --- last 20 lines ---"
                tail -20 "$LOG_DIR/stdlib_e2e_${TIMESTAMP}_run${i}.log" 2>/dev/null | sed 's/^/         /'
                echo "         --- end ---"

                if [[ $STOP_ON_FAIL -eq 1 ]]; then
                    echo ""
                    echo "Stopping on first failure (--stop-on-fail)"
                    break 2
                fi
            fi
        fi
    fi
done

# --- Summary ---

echo ""
echo "========================================================================"
echo "  STRESS TEST SUMMARY"
echo "========================================================================"
echo ""

total_pass=$((sys_thread_pass + stdlib_e2e_pass))
total_fail=$((sys_thread_fail + stdlib_e2e_fail))
total=$((total_pass + total_fail))

if [[ "$TEST_FILTER" == "both" || "$TEST_FILTER" == "sys_thread" ]]; then
    sys_total=$((sys_thread_pass + sys_thread_fail))
    echo "  test_sys_thread:"
    echo "    Passed: $sys_thread_pass / $sys_total"
    echo "    Failed: $sys_thread_fail / $sys_total"
    if [[ ${#sys_thread_signals[@]} -gt 0 ]]; then
        echo "    Failures:"
        for sig in "${sys_thread_signals[@]}"; do
            echo "      - $sig"
        done
    fi
    echo ""
fi

if [[ "$TEST_FILTER" == "both" || "$TEST_FILTER" == "stdlib_e2e" ]]; then
    e2e_total=$((stdlib_e2e_pass + stdlib_e2e_fail))
    echo "  test_rayzor_stdlib_e2e:"
    echo "    Passed: $stdlib_e2e_pass / $e2e_total"
    echo "    Failed: $stdlib_e2e_fail / $e2e_total"
    if [[ ${#stdlib_e2e_signals[@]} -gt 0 ]]; then
        echo "    Failures:"
        for sig in "${stdlib_e2e_signals[@]}"; do
            echo "      - $sig"
        done
    fi
    echo ""
fi

echo "  Total: $total_pass / $total passed"
if [[ $total_fail -gt 0 ]]; then
    fail_rate=$((total_fail * 100 / total))
    echo "  Failure rate: ${fail_rate}% ($total_fail / $total)"
fi
echo ""
echo "  Full logs: $LOG_DIR/"
echo "  Summary:   $SUMMARY_LOG"
echo "========================================================================"

# Append final summary to log
{
    echo ""
    echo "Finished: $(date)"
    echo "Total: $total_pass / $total passed, $total_fail failed"
    if [[ $total_fail -gt 0 ]]; then
        echo "Failure rate: $((total_fail * 100 / total))%"
    fi
} >> "$SUMMARY_LOG"

# Exit with failure if any test failed
if [[ $total_fail -gt 0 ]]; then
    exit 1
fi
