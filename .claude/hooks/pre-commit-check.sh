#!/bin/bash
# Pre-commit check for Rust project
# This hook ensures code quality before committing

set -e

echo "Running pre-commit checks..."

# 1. Format check
echo "[1/4] Checking formatting..."
cargo fmt --all --check
echo "Formatting OK"

# 2. Type check
echo "[2/4] Running cargo check..."
cargo check --all
echo "Check OK"

# 3. Clippy
echo "[3/4] Running clippy..."
cargo clippy --all-targets -- -D warnings
echo "Clippy OK"

# 4. Tests
echo "[4/4] Running tests..."
cargo test --all
echo "Tests OK"

echo "All checks passed!"
