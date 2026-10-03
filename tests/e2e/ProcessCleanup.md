# E2E process cleanup

The Linux real-server runner adopts orphaned descendants as a child subreaper. Each process runs
one case at a time; Make provides parallelism across processes. After a case returns or unwinds,
the harness attempts `stop-all`, kills remaining descendants, and reaps until the kernel confirms
that no children remain. Only then does it delete the case directories. Incomplete process cleanup
retains the directories and prevents later cases from starting in the same runner. Cleanup errors
are reported alongside the original failure. JVM and Gradle homes are isolated so an external
Gradle daemon cannot be reused by a case.

This cleanup is in-process: terminating or crashing the runner itself can leave descendants and
retained directories. Adding concurrent cases inside one runner would require separate process
supervision. `make test` includes isolated subprocess regressions for these cleanup invariants;
the ordinary parallel test harness never enables process-wide child reaping.
