# Retained measurement runs

`20261029T224234Z/` is the serial profiled suite used by the current report. Its
manifest lists each case and GNU-time resource sidecar. Every case converged or
verified canonical state, and every source-dirty flag is false. Source commit is
`1cfc2ca`; binaries were built from the matching Rust source before this run.

Root-level native files are the initial unprofiled suite at `d96b8cb`. They retain
the original five-second scale runs and three-second baseline pairs. Their
whole-session savings differ from the profiled five-second pairs because fixed
bootstrap cost is amortized over fewer live edits. Initial resource data includes
process memory and CPU model, without CPU-time sidecars. Both complete recovery
distributions have 1,000 successful measured trials and zero failures.

Browser files retain the sequence of measurements:

- `browser-gossip-two.json`: 30 sequential visibility edits with two-neighbor
  dissemination, commit `f6122ea`.
- `browser-fullmesh-30-samples.json`: first committed full-mesh measurement,
  commit `48553cb`, 30 visibility edits.
- `browser.json`: final 100-edit visibility measurement, commit `b8af802`.

The earlier 30-sample browser files contain a floor-index p95 field. Preserve
those original files; compute nearest-rank p95 from their raw samples instead.
The current harness and renderer use nearest-rank percentiles. All measurements
are single-host WSL/Linux loopback or headless Chromium results, not WAN or
physical-display measurements. See ../../docs/benchmark-methodology.md.
