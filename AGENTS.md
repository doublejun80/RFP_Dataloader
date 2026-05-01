# AGENTS.md

## Role

You are the implementation agent for this repository.

Your job is to keep implementing the project according to `TASKS.md` until all Priority 1 tasks are complete or a real blocker appears. Do not stop after one small change, one file, one screen, or one task if more eligible work remains.

## Project Goal

Build Tauri RFP v2: a local desktop app that registers RFP PDFs, extracts structure with OpenDataLoader, stores normalized evidence in SQLite, evaluates quality gates, and shows whether the analysis is `검토 필요` or `확정 가능`.

The first milestone is the vertical slice described in:

- `docs/superpowers/plans/2026-05-01-tauri-rfp-v2-vertical-slice.md`

## Read First

At the start of every implementation session:

1. Read `TASKS.md`.
2. Read `IMPLEMENTATION_LOG.md`.
3. Read the current plan under `docs/superpowers/plans/`.
4. Read relevant `spec/*.md` files for the task being implemented.
5. Pick the highest-priority incomplete task that is not blocked.

## Continuous Working Mode

After each task:

1. Run the appropriate verification command.
2. Update `IMPLEMENTATION_LOG.md`.
3. Mark the task status in `TASKS.md`.
4. Continue to the next incomplete task without asking the user to say "continue".

Do not stop for:

- Finishing only one file.
- Finishing only one function.
- Finishing only one screen.
- Finishing only one task while more Priority 1 work remains.
- Needing to choose the next task from `TASKS.md`.
- Needing to run local tests.
- Needing to update docs or logs.

## Stop Only When

Stop only if one of these is true:

1. All Priority 1 tasks in `TASKS.md` are complete.
2. A required secret, API key, paid service, external account, or credential is missing.
3. A product decision cannot be inferred from `spec/`, `TASKS.md`, or the implementation plan.
4. Verification fails and cannot be fixed after two reasonable attempts.
5. The work requires deleting or rewriting large parts of the project without explicit approval.
6. Sandbox or approval policy blocks an essential command and the user declines approval.

## Parallel Agent Policy

Use parallel agents when the user has asked for parallel execution and there are independent workstreams.

Do not parallelize tightly coupled foundation work. For this repository, run these in order:

1. Repository tracking and ignore rules.
2. Tauri scaffold and dependency setup.
3. SQLite schema and migration runner.
4. Document registration.

After the foundation is in place, parallelize independent workstreams with disjoint write scopes:

- OpenDataLoader adapter: `apps/rfp-desktop/src-tauri/src/opendataloader_adapter/`, `apps/rfp-desktop/src-tauri/src/commands/extraction.rs`.
- Block normalizer: `apps/rfp-desktop/src-tauri/src/block_normalizer/`, `fixtures/opendataloader/`.
- Validation and baseline analysis: `apps/rfp-desktop/src-tauri/src/analysis/`, `apps/rfp-desktop/src-tauri/src/validation/`, `apps/rfp-desktop/src-tauri/src/commands/pipeline.rs`.
- Frontend workbench: `apps/rfp-desktop/src/`, `apps/rfp-desktop/vitest.config.ts`.
- Smoke and verification: `apps/rfp-desktop/src-tauri/src/bin/`, `tests/smoke/`, `scripts/verify.sh`.

When dispatching workers:

- Give each worker a clear task and owned files.
- Tell workers they are not alone in the codebase.
- Tell workers not to revert changes made by others.
- Avoid two workers editing the same file at the same time.
- Integrate worker results centrally and run full verification.

## Completion Criteria

A task is complete only when:

- The requested behavior is implemented.
- Related types, commands, DTOs, tests, and docs are updated.
- Relevant verification passes or the reason it cannot pass is recorded.
- `IMPLEMENTATION_LOG.md` states what changed, what was verified, and what remains.
- `TASKS.md` status is updated.

## Verification

Use `scripts/verify.sh` when available.

If a task has a more focused command in the implementation plan, run that focused command first, then run `scripts/verify.sh` when the task is integrated.

Verification priority:

1. Rust focused tests.
2. Rust full tests.
3. Frontend tests.
4. Frontend build.
5. Real PDF smoke when OpenDataLoader and a PDF fixture are available.

## Coding Rules

- Follow the existing specs in `spec/`.
- Prefer small, reviewable logical changes.
- Do not add production dependencies unless necessary.
- If a dependency is necessary, record the reason in `IMPLEMENTATION_LOG.md`.
- Keep UI, Rust commands, SQLite schema, and DTOs consistent.
- Do not hardcode secrets.
- Do not store API keys in SQLite plain text.
- Do not introduce mock-only behavior into production paths.
- Do not revive the v1 PySide6 implementation.
- Treat blockers as `검토 필요`, not as successful completion.

## Reporting

At the end of each work cycle, update `IMPLEMENTATION_LOG.md` with:

- Completed task.
- Files changed.
- Verification command.
- Verification result.
- Remaining task.
- Blockers, if any.

Final user reports should be concise and should include completed work, verification result, and any blocker that needs human action.

