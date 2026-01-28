#define _GNU_SOURCE
#include <signal.h>
#include <execinfo.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <string.h>

static void segfault_handler(int sig, siginfo_t *si, void *unused) {
    void *array[100];
    int size;
    fprintf(stderr, "\n=== SIGNAL %d CAUGHT ===\n", sig);
    fprintf(stderr, "Fault address: %p\n", si->si_addr);
    fprintf(stderr, "SI code: %d\n", si->si_code);
    size = backtrace(array, 100);
    fprintf(stderr, "\nBacktrace (%d frames):\n", size);
    backtrace_symbols_fd(array, size, STDERR_FILENO);
    _exit(128 + sig);
}

__attribute__((constructor))
static void install_handler(void) {
    stack_t ss;
    ss.ss_sp = malloc(65536);
    ss.ss_size = 65536;
    ss.ss_flags = 0;
    sigaltstack(&ss, NULL);
    struct sigaction sa;
    memset(&sa, 0, sizeof(sa));
    sa.sa_flags = SA_SIGINFO | SA_ONSTACK;
    sigemptyset(&sa.sa_mask);
    sa.sa_sigaction = segfault_handler;
    sigaction(SIGSEGV, &sa, NULL);
    sigaction(SIGABRT, &sa, NULL);
    sigaction(SIGBUS, &sa, NULL);
    fprintf(stderr, "[seghandler] Signal handlers installed\n");
}
