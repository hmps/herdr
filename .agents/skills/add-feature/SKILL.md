---
name: add-feature
description: Add a new feature, customization, or local modification to this herdr fork. Use when the user asks to "add", "build", "implement", "wire up", or "customize" anything in this repo. Enforces the fork rule — keep upstream files clean, prefer additive changes, surface upstream-worthy work for upstreaming instead of carrying it locally.
---

# Adding features to a herdr fork

This repo is a personal fork of [ogulcancelik/herdr](https://github.com/ogulcancelik/herdr). `origin` is the fork (`hmps/herdr`); `upstream` is the original. We stay close to upstream and merge `upstream/master` regularly via `/update-herdr`.

**That fork relationship is the single most important constraint when adding features here.** Every merge from upstream is cheaper when local diffs are small and contained. Drift in upstream files is the thing that creates conflicts later — drift in *new* files almost never does.

## The fork rule

Before writing code, decide which bucket the change belongs in:

1. **Upstreamable** — generally useful, fits herdr's product direction.
   → Plan to PR it to `ogulcancelik/herdr`. Implement it the way upstream would accept it (clean, tested, follows their conventions). Land it locally on a branch, but mention upstreaming in the summary.
2. **Personal-only** — niche to this user's workflow, opinionated, experimental, or won't fit upstream's scope.
   → Implement it with **minimal blast radius on upstream files**. Prefer new modules, new files, additive enums/match arms. Avoid reshaping existing types or refactoring upstream code.
3. **Unsure** — ask the user once, briefly. Default to treating it as upstreamable if the feature is well-scoped and broadly useful.

Surface this classification at the top of the plan. The user can correct it.

## Minimal-diff principles (especially for personal-only changes)

In order of preference:

1. **Add a new file/module.** A new `src/<area>/<feature>.rs` registered in one `mod.rs` line is the cheapest thing to maintain across merges.
2. **Add to an enum / match arm / struct field.** Small additive edits to upstream files rarely conflict.
3. **Wrap, don't rewrite.** If you need to alter behavior, wrap the upstream call site rather than editing the upstream function body.
4. **Feature-flag if intrusive.** A `cfg!` gate or a runtime config flag keeps the upstream path identical when the feature is off.
5. **Last resort: edit upstream code in place.** Only when 1–4 don't fit. Keep the edit small, surgical, and easy to spot in `git diff upstream/master..HEAD`.

Avoid:
- Renaming upstream symbols.
- Reordering imports, fields, or match arms in upstream files.
- Reformatting unrelated lines (rustfmt edits to untouched code).
- Refactors that move upstream code around without a behavior reason.
- "While I'm here" cleanups in upstream files.

Each of those creates merge conflicts for zero functional gain.

## Workflow

1. **Clarify** — use AskUserQuestion if scope, classification, or interaction is ambiguous. One round of questions max; don't interrogate.
2. **Locate** — find the closest existing pattern in the codebase (similar dialog, similar pane action, similar config option). Reuse its shape.
3. **Plan** — write a short plan that names:
   - the classification (upstreamable / personal-only)
   - new files to add
   - upstream files to touch (justify each one)
   - tests to add or extend
4. **Implement** — make the changes. Keep the diff tight.
5. **Validate** — run `just check` before declaring done. Add unit tests next to the code (`#[cfg(test)] mod tests`); use `AppState::test_new()` / `Workspace::test_new()` so tests don't need real PTYs.
6. **Summarize** — list the files touched, separate "new files" from "upstream files edited", and note whether this should be upstreamed.

## Repo conventions (from AGENTS.md)

These apply to every change, fork or not:

- **State vs runtime split.** `AppState` is pure data, testable without PTYs/async. `PaneState` ≠ `PaneRuntime`. Don't merge them.
- **Render is pure.** `compute_view()` does geometry; `render()` only draws. Never mutate state during render.
- **No god objects.** If a module is doing too many things, split it.
- **Platform code lives in `src/platform/`.** Don't sprinkle `#[cfg(target_os = ...)]` across core modules.
- **Detection is decoupled.** The detector reads a screen snapshot; it never touches the parser or viewport state.
- **UI consistency.** New dialogs/screens follow existing modal patterns, affordances, and close actions — herdr is mouse-first. Don't invent one-off screens.
- **No `unwrap()`** in production code. Use `tracing` for logging. `#[allow(...)]` only with a comment explaining why.
- **No `CHANGELOG.md` edits** during feature work — that's done at release time.
- **Conventional commits**, lowercase, no emojis. Propose the message and get alignment before committing.

## Common feature shapes

### New CLI subcommand
- Define the subcommand and its args in `src/cli.rs` (additive — new variant in the relevant enum).
- Implement the handler in a new module under `src/<area>/`.
- Wire the dispatch in one place in `cli.rs`.
- Add a unit test for the handler.

### New keybinding / pane action
- Add the action variant to the appropriate enum in `src/input/` or `src/app/`.
- Implement the behavior next to similar actions (look at how an existing nearby action is structured).
- Register the default keybind in the config defaults.
- Document the binding in the keybind help screen.

### New UI screen / dialog
- Look at an existing modal (onboarding, settings, post-update) and follow its structure.
- Put the new screen in `src/ui/` or `src/app/`.
- Reuse close-action conventions; don't invent a new escape pattern.

### New socket / IPC command
- Add the request/response types in the existing protocol module.
- Implement the handler near the related commands; keep grouping consistent.
- Update `SOCKET_API.md` if the surface is documented there.

### Platform-specific behavior
- New code goes under `src/platform/<os>/`.
- The core module calls a platform abstraction; the `#[cfg]` lives inside `platform/`, not in core.

### Personal-only experiments
- Consider a dedicated module under `src/<area>/local/` or similar to make it visually obvious in `git diff upstream/master..HEAD` what is fork-only.
- If the feature can be toggled, add a config flag so the default behavior matches upstream.

## Multi-agent / worktree note

For bigger features, follow the worktree layout in `AGENTS.md`:
- shared checkout: `../herdr`
- task worktrees: `../herdr-worktrees/<task-slug>`
- task branches: `issue/<id>-<slug>` when an issue exists

If you're already inside a task worktree, keep using it — don't nest.

## After the change

- Run `just check`.
- If you want it on your global CLI: `just install-local`.
- Push to `origin`: `git push origin <branch>`.
- If it was classified upstreamable: open a PR against `ogulcancelik/herdr` from your fork.
