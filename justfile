# Root justfile -- run `just --list` to see all recipes.
#
# This file was seeded by .blueprints/setup.sh. Customize the targets below
# for your project. Uncomment the language blocks that apply.
#
# Recipe naming follows .blueprints/base/script-naming.md:
#   check      -- type-check only, no artifacts
#   lint       -- run linter WITH autofix (modifies working tree)
#   lint-check -- run linter without fixing (for CI / pre-push)
#   fmt        -- format code (modifies working tree)
#   fmt-check  -- verify formatting without writing
#   test       -- run tests
#   build      -- produce artifacts
#   ci         -- aggregate: fmt-check check lint-check test build

# Default recipe: show the list of available recipes.
default:
    @just --list

# Re-run the blueprints setup script.
setup:
    .blueprints/setup.sh --detect

# Update the blueprints submodule to the latest upstream commit.
update-blueprints:
    git submodule update --remote .blueprints
    @echo "Blueprints updated. Review changes and commit."

# --- Common targets -----------------------------------------------------------
# Fill these in with your project's actual commands. The examples below show
# concrete per-language wiring.

# Aggregate: what CI runs. Uses non-fixing variants.
ci: fmt-check check lint-check test build

# Compile-only gate. Builds the tests without running them.
check:
    cargo nextest run --all-targets --all-features --no-run

# Run linter with autofix (modifies working tree).
lint:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features -- -D warnings

# Run linter without fixing (CI-safe).
lint-check:
    cargo clippy --all-targets --all-features -- -D warnings

# Format code (modifies working tree).
fmt:
    cargo fmt --all

# Verify formatting without writing (CI-safe).
fmt-check:
    cargo fmt --all -- --check

# Run tests. Nextest does not run doctests, but this crate is a binary with no
# library target, so there are none to run. Add `cargo test --doc` here if that
# ever changes.
test:
    cargo nextest run --all-features

# Build the project.
build:
    cargo build --all-features

# Sign a PDF with the debug binary.
run *args:
    cargo run -- {{args}}

# --- Language examples (uncomment the ones you need) --------------------------
#
# Rust:
#   check:
#       cargo check --all-targets --all-features
#   lint:
#       cargo clippy --fix --allow-dirty --allow-staged --all-targets --all-features -- -D warnings
#   lint-check:
#       cargo clippy --all-targets --all-features -- -D warnings
#   fmt:
#       cargo fmt
#   fmt-check:
#       cargo fmt -- --check
#   test:
#       cargo test --all-features
#   build:
#       cargo build --release
#
# TypeScript:
#   check:
#       npm run check       # root script: "tsc --noEmit"
#   lint:
#       npx biome lint --write .
#   lint-check:
#       npx biome lint .
#   fmt:
#       npx biome format --write .
#   fmt-check:
#       npx biome format .
#   test:
#       npm test
#   build:
#       npm run build
#
# Python:
#   check:
#       uv run mypy .
#   lint:
#       uv run ruff check --fix .
#   lint-check:
#       uv run ruff check .
#   fmt:
#       uv run ruff format .
#   fmt-check:
#       uv run ruff format --check .
#   test:
#       uv run pytest
#   build:
#       uv build
#
# Go:
#   check:
#       go vet ./...
#   lint:
#       staticcheck ./...
#   lint-check:
#       staticcheck ./...
#   fmt:
#       gofmt -w .
#   fmt-check:
#       test -z "$(gofmt -l .)"
#   test:
#       CGO_ENABLED=1 go test -v ./...
#   build:
#       go build ./...

# --- Spec-driven development (uncomment if you ran setup.sh --domain ...) ------
#
# Scaffold a new feature surface: copy the artifact template into the next
# specs/NNN-name/ and point .specify/feature.json at it.
#   just new-feature my-feature-name
#
# new-feature name:
#     #!/usr/bin/env bash
#     set -euo pipefail
#     last=$(ls -1d specs/[0-9][0-9][0-9]-*/ 2>/dev/null | sed 's#.*/##;s#-.*##' | sort -n | tail -1)
#     next=$(printf '%03d' $((10#${last:-0} + 1)))
#     dir="specs/${next}-{{name}}"
#     cp -r .blueprints/templates/spec-driven/feature "$dir"
#     mkdir -p .specify
#     printf '{\n  "feature_directory": "%s"\n}\n' "$dir" > .specify/feature.json
#     echo "Created $dir; .specify/feature.json now points at it."

# --- Visual recap (uncomment if you ran setup.sh --domain visual-recap) --------
#
# Upsert a system-recap block into a PR description. Normally the agent calls
# the script directly; this recipe is for doing it by hand.
#   just recap 42 /tmp/recap-block.md
#
# recap pr block:
#     ./scripts/bp-upsert-recap-block.sh {{pr}} {{block}}
