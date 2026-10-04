# Persistent compiler learning

Compiler learning is advisory state, not compiler authority.

Each learning entry records a diagnostic pattern and occurrence count. Entries can additionally be marked `verified-repair` only through an explicit caller action after a repair has been verified.

The persisted format is versioned as `ARDISA-LEARNING-V2` and remains backward-readable from V1 records. Older records are loaded as observed rather than promoted to verified knowledge.

## Promotion rule

Observed -> VerifiedRepair requires:

1. the diagnostic was reproduced;
2. a repair was applied;
3. the repaired program passed the relevant compiler checks;
4. the caller explicitly recorded the verified repair.

Learning does not bypass tests, CI, ownership, security, or acceptance gates.
