# Compatibility reproducer policy

Keep each issue-specific reproducer under `safe/tests/compat/<issue>-<slug>/` and
register a matching explicit make target named
`compat-<issue>-<slug>-regress` in `safe/tests/Makefile` by appending it to
`COMPAT_REGRESS_TARGETS`. `compat-regress` is the only aggregate entry point,
so every new reproducer must be wired into that list instead of discovered by
file scanning.

Reproducers in this directory must stay local, deterministic, and minimal. They
may consume committed inputs from `original/tests/`, `original/pic/`,
`safe/tests/malformed/`, and other in-repo authorities in place, or generate
temporary inputs during the test run. Do not vendor downstream source trees,
copy oracle corpora out of `original/`, depend on the network, or depend on
ambient machine state.

A tiny local fixture or expected-output file is acceptable only when it is the
smallest practical artifact that captures the regression, is authored for the
reproducer rather than copied from upstream/downstream packages, and its
provenance is stated in the reproducer source or an adjacent note. Prefer
plain-text fixtures when possible, keep binary inputs auditable and narrowly
scoped, and document why an existing in-repo oracle was insufficient.
