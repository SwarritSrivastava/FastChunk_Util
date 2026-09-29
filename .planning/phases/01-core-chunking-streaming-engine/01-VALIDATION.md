---
phase: "01"
slug: "core-chunking-streaming-engine"
status: draft
nyquist_compliant: true
wave_0_complete: false
created: "2026-09-30"
---

# Phase 01 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (built-in Rust test harness) |
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
| 01-01-01 | 01 | 1 | FR-1, FR-8 | — | Bounds check on chunk size parsing | unit | `cargo test parse_size` | ❌ W0 | ⬜ pending |
| 01-01-02 | 01 | 1 | FR-2, FR-3 | — | Clean slicing across part boundaries without corrupting streams | unit/integration | `cargo test chunked_writer` | ❌ W0 | ⬜ pending |
| 01-01-03 | 01 | 1 | FR-4 | — | Correct SHA-256 computation per part and manifest schema | unit/integration | `cargo test manifest` | ❌ W0 | ⬜ pending |
| 01-01-04 | 01 | 2 | FR-1..FR-4, FR-8 | — | Path traversal guards on archive extraction / CLI inputs | integration | `cargo test cli_split` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/chunking_tests.rs` — integration tests for chunk boundaries and large file splitting
- [ ] `Cargo.toml` — project and test dependencies

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Real USB fat32 chunk size behavior | FR-1 | Requires physical hardware drive | Test with a test directory using `-s 14G` flag |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify or Wave 0 dependencies
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references
- [x] No watch-mode flags
- [x] Feedback latency < 10s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** approved 2026-09-30
