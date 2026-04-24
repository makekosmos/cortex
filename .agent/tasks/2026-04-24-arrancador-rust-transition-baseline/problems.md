# Problems and Fixes

No blocking verification problems remain.

Minor implementation adjustments:

- The compare script was strengthened to compare by benchmark name, emit direction-aware statuses, support `--candidate` as an alias for `--current`, and hard-fail correctness mismatches.
- The preserved in-tree baseline was verified by SHA256 against the proof-loop raw baseline artifact.
