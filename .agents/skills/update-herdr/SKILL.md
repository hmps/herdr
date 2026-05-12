---
name: update-herdr
description: Bring upstream herdr changes into a customized fork, with preview, selective cherry-pick, conflict resolution, and low token usage.
---

# About

Your herdr fork drifts from upstream (`ogulcancelik/herdr`) as you customize it. This skill pulls upstream changes in without losing your modifications.

Run `/update-herdr` in Claude Code.

## How it works

**Preflight**: checks for a clean working tree (`git status --porcelain`). If the `upstream` remote is missing, asks for the URL (defaults to `https://github.com/ogulcancelik/herdr.git`) and adds it. Detects the upstream branch (`master` or `main`).

**Backup**: creates a timestamped backup branch and tag (`backup/pre-update-<hash>-<timestamp>`, `pre-update-<hash>-<timestamp>`) before touching anything. Safe to run multiple times.

**Preview**: runs `git log` and `git diff` against the merge base. Groups changed files into buckets:
- **Source** (`src/`): may conflict with local customizations
- **Build/config** (`Cargo.toml`, `Cargo.lock`, `build.rs`, `justfile`, `clippy.toml`, `rust-toolchain*`): lockfile changes mean a rebuild and possibly a `cargo update`
- **Vendor** (`vendor/`): vendored libghostty-vt sources; may need `scripts/build_vendored_libghostty_vt.sh`
- **Scripts** (`scripts/`): maintenance scripts and their tests
- **Tests** (`tests/`): integration tests
- **CI** (`.github/`, `.githooks/`): pipeline and hook changes
- **Docs** (`README.md`, `CHANGELOG.md`, `*.md`, `assets/`, `website/`): low risk
- **Other**: miscellaneous

**Update paths** (you pick one):
- `merge` (default): `git merge upstream/<branch>`. One-pass conflict resolution.
- `cherry-pick`: `git cherry-pick <hashes>`. Pull in only the commits you want.
- `rebase`: `git rebase upstream/<branch>`. Linear history, but conflicts resolve per-commit.
- `abort`: just view the changelog, change nothing.

**Conflict preview**: dry-run merge (`git merge --no-commit --no-ff` then `--abort`) to list conflicting files before you commit.

**Conflict resolution**: opens only conflicted files, resolves the conflict markers, keeps local customizations intact. Also watches for semantic collisions git auto-merge can't see (shared APIs both sides changed; positional indices into a list both sides modified).

**Fail-fast build**: runs `cargo build --locked` immediately after the merge to surface duplicate definitions, missing args, or missing fields — the kind of breakage git auto-merges into existence without conflict markers.

**Validation**: runs `just check` (lint + tests + script tests). Falls back to `cargo build --locked` + `cargo nextest run --locked` if `just` is unavailable.

**Breaking changes check**: after validation, diffs `CHANGELOG.md` for new entries marked `[BREAKING]` and surfaces them.

## Rollback

The backup tag is printed at the end of each run:
```
git reset --hard pre-update-<hash>-<timestamp>
```

The backup branch `backup/pre-update-<hash>-<timestamp>` also exists.

## Token usage

Only opens files with actual conflicts. Uses `git log`, `git diff`, and `git status` for everything else. Does not scan or refactor unrelated code.

---

# Goal
Help a user with a customized herdr fork safely incorporate upstream changes without losing local work and without blowing tokens.

# Operating principles
- Never proceed with a dirty working tree.
- Always create a rollback point (backup branch + tag) before touching anything.
- Prefer git-native operations (fetch, merge, cherry-pick). Do not manually rewrite files except conflict markers.
- Default to MERGE (one-pass conflict resolution). Offer REBASE only on explicit request.
- Keep token usage low: rely on `git status`, `git log`, `git diff`, and open only conflicted files.

# Step 0: Preflight (stop early if unsafe)

Run:
- `git status --porcelain`

If output is non-empty, tell the user to commit or stash first, then stop.

Confirm remotes:
- `git remote -v`

If `upstream` is missing:
- Ask the user for the upstream repo URL (default: `https://github.com/ogulcancelik/herdr.git`).
- Add it: `git remote add upstream <user-provided-url>`

Determine the upstream branch:
- `git fetch upstream --prune`
- `git branch -r | grep upstream/`
- If `upstream/master` exists, use `master`.
- Else if `upstream/main` exists, use `main`.
- Otherwise, ask the user which branch to track.
- Store as `UPSTREAM_BRANCH` for all subsequent commands. Every command below referencing `upstream/master` should use `upstream/$UPSTREAM_BRANCH` instead.

# Step 1: Create a safety net

Capture current state:
- `HASH=$(git rev-parse --short HEAD)`
- `TIMESTAMP=$(date +%Y%m%d-%H%M%S)`

Create backup branch and tag:
- `git branch backup/pre-update-$HASH-$TIMESTAMP`
- `git tag pre-update-$HASH-$TIMESTAMP`

Save the tag name for later reference in the summary and rollback instructions.

# Step 2: Preview what upstream changed (no edits yet)

Compute the common base:
- `BASE=$(git merge-base HEAD upstream/$UPSTREAM_BRANCH)`

Show upstream commits since `BASE`:
- `git log --oneline $BASE..upstream/$UPSTREAM_BRANCH`

Show local commits since `BASE` (your custom drift):
- `git log --oneline $BASE..HEAD`

Show file-level impact from upstream:
- `git diff --name-only $BASE..upstream/$UPSTREAM_BRANCH`

Bucket the changed files:
- **Source** (`src/`): may conflict with local customizations
- **Build/config** (`Cargo.toml`, `Cargo.lock`, `build.rs`, `justfile`, `clippy.toml`, `rust-toolchain*`): lockfile changes mean a rebuild and possibly a `cargo update`
- **Vendor** (`vendor/`): vendored libghostty-vt sources; may need `scripts/build_vendored_libghostty_vt.sh`
- **Scripts** (`scripts/`): maintenance scripts and their tests
- **Tests** (`tests/`): integration tests
- **CI** (`.github/`, `.githooks/`): pipeline and hook changes
- **Docs** (`README.md`, `CHANGELOG.md`, `*.md`, `assets/`, `website/`): low risk
- **Other**: miscellaneous

Present the buckets and ask the user to pick a path with AskUserQuestion:
- A) **Full update**: merge all upstream changes (default)
- B) **Selective update**: cherry-pick specific upstream commits
- C) **Abort**: only wanted the preview
- D) **Rebase mode**: linear history (warn: resolves conflicts per-commit)

If Abort: stop here.

# Step 3: Conflict preview (before committing anything)

If Full update or Rebase:
- Dry-run merge to preview conflicts as a single chained command so the abort always runs:
  ```
  git merge --no-commit --no-ff upstream/$UPSTREAM_BRANCH; git diff --name-only --diff-filter=U; git merge --abort
  ```
- If conflicts were listed, show them and ask the user whether to proceed.
- If clean, say so and proceed.

# Step 4A: Full update (MERGE, default)

Run:
- `git merge upstream/$UPSTREAM_BRANCH --no-edit`

If conflicts occur:
- Run `git status` and identify conflicted files.
- For each conflicted file:
  - Open the file.
  - Resolve only the conflict markers.
  - Preserve intentional local customizations.
  - Incorporate upstream fixes/improvements.
  - Do not refactor surrounding code.
  - `git add <file>`
- Watch for **semantic collisions** — these don't show as conflict markers but matter:
  - **Shared API / endpoint**: if both sides changed the behavior of the same method, route, or CLI subcommand (e.g., both bound new features to `pane.rename`), pick one semantics and update the other side's tests/docs to match. Auto-merge will happily keep both field definitions even when only one behavior survives.
  - **Positional index drift**: if both sides modified the same array/vec literal (especially `optional_bindings`-style lists), indices used elsewhere may be silently wrong. Re-derive any `foo[N]` references that point into a list both sides touched.
- When all resolved, if the merge did not auto-commit: `git commit --no-edit`.

# Step 4B: Selective update (CHERRY-PICK)

If user chose Selective:
- Recompute `BASE` if needed: `BASE=$(git merge-base HEAD upstream/$UPSTREAM_BRANCH)`
- Show the commit list again: `git log --oneline $BASE..upstream/$UPSTREAM_BRANCH`
- Ask which commit hashes they want.
- Apply: `git cherry-pick <hash1> <hash2> ...`

On conflicts:
- Resolve markers, then:
  - `git add <file>`
  - `git cherry-pick --continue`
- If the user wants to stop: `git cherry-pick --abort`.

# Step 4C: Rebase (only if user explicitly chose option D)

Run:
- `git rebase upstream/$UPSTREAM_BRANCH`

On conflicts:
- Resolve markers, then:
  - `git add <file>`
  - `git rebase --continue`
- If it gets messy (more than 3 rounds of conflicts):
  - `git rebase --abort`
  - Recommend merge instead.

# Step 4.5: Refresh dependencies (if Cargo.lock or Cargo.toml changed)

Check whether the update touched the manifest or lockfile:
- `git diff <backup-tag-from-step-1>..HEAD --name-only | grep -E '^(Cargo\.toml|Cargo\.lock)$'`

If `Cargo.toml` changed but `Cargo.lock` did not, run `cargo update -p herdr --offline` to refresh the lockfile entry for this package without hitting the network.

Otherwise skip — `just check` / `cargo build` will pull what they need.

# Step 4.7: Catch silent auto-merge duplicates (fail-fast build)

Before running the full `just check`, run a quick build to surface issues git's auto-merge couldn't see:
- `cargo build --locked`

Git's "Auto-merging" succeeds with no conflict markers when both sides add similar content in the same region. This produces duplicate definitions that only the compiler catches. Common patterns from past runs:
- Duplicate `fn` / `struct` / `enum` declared twice (E0428)
- Duplicate struct field (E0124) — e.g., the same field name added by both sides in a struct that auto-merged
- Duplicate match arm — unreachable-pattern warning, but a real bug
- Signature change on one side + new caller on the other → missing argument (E0061)
- Both sides added a new field to the same struct → callers from one side miss the other's field (E0063)

If `cargo build` fails: fix only the merge-caused issues (remove the obvious duplicate, add the missing field/arg). Do not refactor. Re-run `cargo build` until it's green, then proceed to Step 5.

If it builds clean, proceed.

# Step 5: Validation

Capture what changed for downstream decisions:
- `CHANGED_FILES=$(git diff --name-only <backup-tag-from-step-1>..HEAD)`

**Primary check** (preferred, matches CI): `just check`

If `just` is not on PATH, fall back to running the equivalent steps directly:
- `cargo fmt --check`
- `cargo clippy --all-targets --locked -- -D warnings`
- `cargo nextest run --locked --status-level fail --final-status-level fail --failure-output final --success-output never`
- `python3 -m unittest scripts.test_changelog scripts.test_vendor_libghostty_vt`

**Vendor rebuild** (only if any `vendor/` files are in `CHANGED_FILES`):
- `scripts/build_vendored_libghostty_vt.sh`

If validation fails:
- Show the error.
- Only fix issues clearly caused by the merge (missing imports, type mismatches, formatting from merged code).
- Do not refactor unrelated code.
- If unclear, ask the user before making changes.

# Step 6: Breaking changes check

After validation succeeds, diff `CHANGELOG.md` against the backup tag:
- `git diff <backup-tag-from-step-1>..HEAD -- CHANGELOG.md`

Scan added lines for `[BREAKING]` markers. If none, skip silently.

If any are found:
- Show a header: "This update includes breaking changes that may require action:"
- List each entry verbatim.
- Ask the user whether they want to handle them now or later. Do not invent migration scripts.

# Step 7: Summary + next steps

Show:
- Backup tag: the tag name created in Step 1
- New HEAD: `git rev-parse --short HEAD`
- Upstream HEAD: `git rev-parse --short upstream/$UPSTREAM_BRANCH`
- Conflicts resolved (list files, if any)
- Breaking changes flagged (if any)
- Remaining local diff vs upstream: `git diff --name-only upstream/$UPSTREAM_BRANCH..HEAD`

Tell the user:
- To rollback: `git reset --hard <backup-tag-from-step-1>`
- Backup branch also exists: `backup/pre-update-<HASH>-<TIMESTAMP>`
- To ship the updated binary to their global install: `just install-local`
  (skip this hint if neither `src/` nor `Cargo.*` were in `CHANGED_FILES`)
- To push the update to their fork: `git push origin <current-branch>`
