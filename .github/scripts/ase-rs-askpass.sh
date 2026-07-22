#!/usr/bin/env bash
# GIT_ASKPASS helper for the CTO-approved (2026-07-21) ase-rs auth path.
#
# Background: `Sou-Tokuda/ase-rs` is a PRIVATE repo (gh api -> private:true).
# Cargo.lock pins the 4 workspace members (ase-driver / ase-types / ase-dsn /
# ase-tds) to ase-rs rev `2bc35515`, and `schema-diff` is a workspace member,
# so every `cargo --workspace` op resolves that git source. On CI the runner's
# default `GITHUB_TOKEN` is scoped to the tsqlremaker repo and CANNOT read the
# foreign private ase-rs repo. The prior `persist-credentials: false` fix
# removed the only credential entirely and failed with
# `could not read Username` (exit 128) — see PR #201 run logs.
#
# Resolution (CTO option (a)): expose an ase-rs read-access PAT as the repo
# secret `CARGO_ASE_RS_TOKEN`, route cargo git fetches through the system git
# CLI (`net.git-fetch-with-cli = true` in `.cargo/config.toml`), and point git
# at this script via `GIT_ASKPASS`. When git needs a credential for
# github.com it invokes this script with a prompt string; we answer the
# password prompt with the PAT and the username prompt with `x-access-token`
# (the conventional PAT username).
#
# Scope: minimum-privilege. The secret is read from the environment and only
# ever emitted to stdout for a github.com prompt. It is never logged.
set -u

# Only answer for github.com (ase-rs host). Stay silent for any other host so
# unrelated git operations are unaffected.
case "$1" in
  *github.com*) ;;
  *) exit 0 ;;
esac

case "$1" in
  Username*) echo "x-access-token" ;;
  Password*) printf '%s' "${CARGO_ASE_RS_TOKEN:-}" ;;
  *) exit 0 ;;
esac
