# Synthetic process workload

`workload_ps.txt` is deterministic test data, not a saved process inventory.
It contains 850 rows, 585 command groups, duplicate commands, long paths, and
paths containing spaces. The CPU and memory values are synthetic. The timing
tests use this to exercise grouping, sorting and rendering at a realistic size;
it does not establish performance on any particular host.

The earlier `real_ps.txt` was replaced for public-release preparation. Earlier development
commits remain in a separate private archive; the public repository starts from
this synthetic workload and does not import that history.
