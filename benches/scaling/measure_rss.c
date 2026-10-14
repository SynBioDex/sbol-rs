/* Execute one process and record wait4() peak resident memory. */
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: measure_rss <output.json> <command> [args...]\n");
        return 2;
    }
    pid_t child = fork();
    if (child < 0) { perror("fork"); return 2; }
    if (child == 0) {
        execvp(argv[2], &argv[2]);
        perror("execvp");
        _exit(127);
    }
    int status;
    struct rusage usage;
    while (wait4(child, &status, 0, &usage) < 0) {
        if (errno != EINTR) { perror("wait4"); return 2; }
    }
    int code = WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
    FILE *output = fopen(argv[1], "w");
    if (!output) { perror("fopen"); return 2; }
#ifdef __APPLE__
    long peak_rss_kib = usage.ru_maxrss / 1024; /* macOS reports bytes. */
#else
    long peak_rss_kib = usage.ru_maxrss; /* Linux reports KiB. */
#endif
    fprintf(output, "{\"peak_rss_kib\":%ld,\"exit_code\":%d}\n", peak_rss_kib, code);
    fclose(output);
    return code;
}
