#!/bin/sh
# Full gate: fmt, build, tests, clippy (-D warnings). Run from anywhere in the repo.
cd "$(git rev-parse --show-toplevel)" || exit 1
cargo fmt --all
cargo build 2>&1 | grep -E "^(error|warning)" -A10 | head -80
cargo test 2>&1 | grep -E "test result|FAILED|panicked|^error" -A3 | grep -v " 0 passed" | head -30
cargo clippy --all-targets -- -D warnings 2>&1 | grep -E "^(error|warning)" -A10 | head -80
echo CHECK_DONE
