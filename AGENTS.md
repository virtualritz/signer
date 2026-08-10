# Repository Guidelines

This file provides guidance to Claude Code and other AI agents working in this repository.

## Project Context

A single Rust binary that writes a date and a signature image into the blanks
following two labels in a PDF form. A blank is the run of underscores after a
label, so a form is filled by matching text, not by coordinates.

The one architectural decision worth knowing: **`pdf_oxide` reads and `lopdf`
writes.** Reading needs per-character bounding boxes to find the underscore
runs, and `pdf_oxide` is alone in reporting them. Writing cannot use it, because
as of 0.3.77 its page overlay inherits the original content stream's transform
instead of starting in page space, emits `BT` with no closing `ET`, and
references an image XObject it never registers. Do not try to consolidate onto
one crate without re-testing against a PDF whose content stream ends with a
non-identity CTM.

`src/main.rs` is the command line and config; `src/stamp/` locates the blanks
and writes the overlay.

## Build, Test, and Development Commands

```bash
just ci      # fmt-check + check + lint-check + test + build
just test    # cargo nextest run --all-features
just run -- --help
```

There is no library target, so there are no doctests.

## Blueprint References

For cross-project standards, see `.blueprints/`:

### Core Rules (MUST READ)

- [Agent Behavior Rules](.blueprints/base/AGENTS.md)
- [Context Economy](.blueprints/base/context-economy.md)
- [Script and Recipe Naming](.blueprints/base/script-naming.md)
- [File Size Limits](.blueprints/base/file-size.md)
- [Git Safety](.blueprints/base/git-safety.md)
- [Test Ownership](.blueprints/base/test-ownership.md)
- [API Change Protocol](.blueprints/base/api-changes.md)

<!-- CUSTOMIZE: Uncomment the language sections that apply to this project. -->

### Language: Rust

- [Rust Agent Rules](.blueprints/lang/rust/AGENTS.md)
- [Rust Testing](.blueprints/lang/rust/testing.md)

<!-- ### Language: TypeScript
- [TypeScript Agent Rules](.blueprints/lang/typescript/AGENTS.md)
- [TypeScript Testing](.blueprints/lang/typescript/testing.md) -->

<!-- ### Language: Python
- [Python Agent Rules](.blueprints/lang/python/AGENTS.md)
- [Python Testing](.blueprints/lang/python/testing.md) -->

<!-- ### Language: Go
- [Go Agent Rules](.blueprints/lang/go/AGENTS.md)
- [Go Testing](.blueprints/lang/go/testing.md) -->

### Domain

- [Visual Regression Testing](.blueprints/domain/visual-regression.md)
- [Spec-Driven Development](.blueprints/domain/spec-driven-development.md)
- [Domain Glossary](.blueprints/domain/glossary.md)

### Task-Gated (read only when the task calls for it)

- [Documentation Structure](.blueprints/base/doc-structure.md) -- read **before** writing, restructuring, or reviewing user-facing documentation. Not needed for ordinary code changes.
- [Measurement Discipline](.blueprints/base/measurement-discipline.md) -- read before auditing, classifying failures, or writing down a number someone will act on.
- [False Green](.blueprints/base/false-green.md) -- read when the suite is green but you do not trust it, or before repairing a defect other code may depend on.
- [Thresholds and Tolerances](.blueprints/base/thresholds.md) -- read before tuning a tolerance, cap, or magic constant.

<!-- CUSTOMIZE: uncomment the next line when the visual-recap domain is enabled. -->
<!-- - [Visual Recap](.blueprints/domain/visual-recap.md): read **when creating or updating a pull request** whose changed paths match a primitive in `docs/architecture/primitives.yaml`. If nothing matches, no recap block is produced and this file is not needed. -->

### Reference

- [Writing Style](.blueprints/base/writing-style.md)
- [Documentation Standards](.blueprints/base/documentation.md)
- [Commit Messages](.blueprints/base/commit-messages.md)
- [Defensive Programming](.blueprints/base/defensive-programming.md)
- [Error Recovery](.blueprints/base/error-recovery.md)

## Project-Specific Rules

- **Never commit a PDF.** The documents this tool is written for are care
  records holding a named person's address, insurance number and medication
  schedule, and the signed outputs additionally embed a real handwritten
  signature. `.gitignore` excludes `*.pdf`; do not add exceptions.
- Errors use `anyhow` rather than a `thiserror` type. This is a binary, so
  errors are read by a person and never matched on by a caller.
