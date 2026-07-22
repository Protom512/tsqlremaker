#!/usr/bin/env bash
# Verification harness for Task 3 (PR #201 / #194 CI auth wiring).
#
# Asserts the CTO-approved (2026-07-21) PAT auth mechanism is present on
# EVERY job of all 3 workflows, and that the broken `persist-credentials:
# false` approach (false premise: ase-rs is private, not public-read) is gone.
#
# CTO decision: option (a) — repo secret `CARGO_ASE_RS_TOKEN` (ase-rs
# read-access PAT) exposed via a GIT_ASKPASS helper script, with
# `net.git-fetch-with-cli = true` so cargo shells out to git (which honors
# GIT_ASKPASS). Auth is NOT feature-gated: Cargo.lock pins the 4 ase crates
# to the ase-rs git rev, and schema-diff is a workspace member, so every
# `cargo --workspace` op resolves the locked git source on every job.
#
# Run: bash .github/workflows/test-auth-wiring.sh
set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CI="$ROOT/.github/workflows/ci.yml"
RUST="$ROOT/.github/workflows/rust.yml"
CODECOV="$ROOT/.github/workflows/codecov.yml"
CARGO_CFG="$ROOT/.cargo/config.toml"
ASKPASS="$ROOT/.github/scripts/ase-rs-askpass.sh"

pass=0
fail=0
ok()   { pass=$((pass+1)); echo "PASS: $1"; }
bad()  { fail=$((fail+1)); echo "FAIL: $1"; }

# --- Condition 1: broken persist-credentials:false directive must be gone ---
# Match the YAML key (`persist-credentials:` with colon), not the word in
# prose comments that document why it was removed.
for f in "$CI" "$RUST" "$CODECOV"; do
  if grep -Eq '^\s*persist-credentials\s*:' "$f"; then
    bad "$f still has a persist-credentials: directive (false premise — ase-rs is PRIVATE)"
  else
    ok "$f has no persist-credentials: directive"
  fi
done

# --- Condition 2: git-fetch-with-cli must be set in .cargo/config.toml ---
if [[ -f "$CARGO_CFG" ]] && grep -q "git-fetch-with-cli = true" "$CARGO_CFG"; then
  ok ".cargo/config.toml has net.git-fetch-with-cli = true"
else
  bad ".cargo/config.toml missing git-fetch-with-cli = true (cargo must shell out to git for GIT_ASKPASS)"
fi

# --- Condition 3: askpass helper script exists + reads the secret ---
if [[ -f "$ASKPASS" ]] && grep -q "CARGO_ASE_RS_TOKEN" "$ASKPASS"; then
  ok "askpass helper exists and references CARGO_ASE_RS_TOKEN"
else
  bad "askpass helper missing at $ASKPASS"
fi

# --- Condition 4: the secret env is wired into every workflow ---
for f in "$CI" "$RUST" "$CODECOV"; do
  if grep -q "CARGO_ASE_RS_TOKEN" "$f"; then
    ok "$(basename "$f") wires CARGO_ASE_RS_TOKEN"
  else
    bad "$(basename "$f") does not reference CARGO_ASE_RS_TOKEN"
  fi
done

# --- Condition 5: GIT_ASKPASS env var points at the helper in every workflow ---
for f in "$CI" "$RUST" "$CODECOV"; do
  if grep -q "GIT_ASKPASS" "$f"; then
    ok "$(basename "$f") sets GIT_ASKPASS"
  else
    bad "$(basename "$f") does not set GIT_ASKPASS"
  fi
done

# --- Condition 6: auth is NOT gated on a feature flag ---
# (Every cargo --workspace op resolves the locked ase-rs git rev, so the
#  secret must be available on jobs that run plain `cargo build`, not only
#  those that pass --features ase / --all-features.)
if grep -q "if:.*features.*ase\|ASE.*&&" "$CI" 2>/dev/null; then
  bad "ci.yml appears to conditionally gate auth on a feature flag"
else
  ok "ci.yml does not feature-gate the auth setup"
fi

# --- Condition 7: GIT_TERMINAL_PROMPT=0 on every workflow (T-WINDOWS-1) ---
# Windows runners fail the ase-rs git fetch with:
#   fatal: Cannot prompt because user interactivity has been disabled.
#   bash: line 1: /dev/tty: No such file or directory
#   error: failed to execute prompt script (exit code 1)
# Root cause: with GIT_TERMINAL_PROMPT unset, git-for-Windows falls back to a
# TTY prompt when a credential is needed; a Windows CI runner has no
# controlling terminal (/dev/tty absent), so the prompt script fails with exit
# 1 and the fetch aborts (exit 128) BEFORE GIT_ASKPASS can supply the PAT.
# Setting GIT_TERMINAL_PROMPT=0 forces git to rely solely on GIT_ASKPASS and
# never attempt an interactive prompt. This is the load-bearing Windows fix.
for f in "$CI" "$RUST" "$CODECOV"; do
  if grep -Eq '^\s*GIT_TERMINAL_PROMPT\s*:\s*["'\'']?0["'\'']?\s*$' "$f"; then
    ok "$(basename "$f") sets GIT_TERMINAL_PROMPT=0 (Windows askpass guard)"
  else
    bad "$(basename "$f") missing GIT_TERMINAL_PROMPT=0 (Windows /dev/tty blocker — T-WINDOWS-1)"
  fi
done

echo "----"
echo "passed=$pass failed=$fail"
if [[ "$fail" -ne 0 ]]; then
  exit 1
fi
