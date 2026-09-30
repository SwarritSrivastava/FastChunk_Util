---
phase: "02"
slug: "zero-dependency-setup-fast-restorer"
status: draft
nyquist_compliant: true
wave_0_complete: false
created: "2026-09-30"
---

# Phase 02 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test harness) + shell validation |
| **Config file** | Cargo.toml |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd-verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Threat Ref | Secure Behavior | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|------------|-----------------|-----------|-------------------|-------------|--------|
| 02-01-01 | 01 | 1 | FR-6 | — | Pre-flight missing-part detection halts early | unit | `cargo test preflight` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | FR-7, NFR-1 | — | Chained stream restoration extracts in seconds | unit/integration | `cargo test restore_stream` | ❌ W0 | ⬜ pending |
| 02-01-03 | 01 | 1 | FR-5, NFR-2 | — | Auto-generates restore.sh and restore.bat | integration | `cargo test script_gen` | ❌ W0 | ⬜ pending |
| 02-01-04 | 01 | 1 | FR-5..FR-7 | — | CLI subcommand `restore` roundtrip end-to-end | integration | `cargo test cli_restore` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/restore_tests.rs` — integration tests for missing-part detection, script generation, and extraction
- [ ] `src/restore.rs` — core restoration engine module

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-09-30
