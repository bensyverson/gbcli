# Gotchas

Project-specific traps that cost real time and that no general rule predicts. Read at session start; append when you hit one.

- **Fix at the source when you can.** A gotcha is a bug report on our tooling, not a permanent fact — if it can be fixed in code, file it in `job` and fix it instead of recording it here.
- **Delete anything that becomes obvious, gets fixed, or stops recurring.** Keep this list short; a long list is one nobody reads.
- **Feedback about `AGENTS.md` itself** — a rule that was wrong, misread, or cost time — goes here too, prefixed `rule:`. It is harvested when the shared rules are reviewed.

Format: one dated H2 headline, then one paragraph. If an entry needs more than that, it's a finding — write it as a dated doc under `project/` and link it from the paragraph.

---

## 2026-11-06 — git in the Bash sandbox needs `GIT_CONFIG_GLOBAL=/dev/null`

The sandbox can't read `~/.gitconfig`, so `git clone` and friends fail with "unable to access '/Users/ben/.gitconfig': Operation not permitted". Prefix git commands with `GIT_CONFIG_GLOBAL=/dev/null`; the warning about `~/.config/git/ignore` is harmless. In an isolated agent worktree the harness refuses that prefix ("injecting git configuration whose effect on where git writes can't be verified"); there, run the plain git call with the sandbox off instead (observed 2026-11-06, leaf eeTyfo).

## 2026-11-06 — subagents can't write report files

The harness refuses a subagent's Write to a findings file ("Subagents should return findings as text"). Brief research agents to return findings in their final message, and have the main thread save them.

## 2026-11-06 — `GIT_CONFIG_GLOBAL=/dev/null` is refused in an isolated worktree

A worktree-isolated subagent's harness refuses any git command carrying `GIT_CONFIG_GLOBAL` ("injecting git configuration whose effect on where git writes can't be verified"), so the entry above doesn't work there. Run the plain git command with the sandbox disabled for that one call instead, as `project/agents/harness.md` says.

## 2026-11-06 — Python downloads through the sandbox proxy end early, silently

`urllib` through the Bash sandbox's filtering proxy stopped short of `Content-Length` on most archives (e.g. 409,600 of 423,779 bytes, every attempt) with no exception, or raised `IncompleteRead`; `curl` through the same proxy got whole files. `scripts/corpus/fetch.py` checks the length and resumes with HTTP `Range`; anything else that downloads in Python needs the same.
