# Gotchas

Project-specific traps that cost real time and that no general rule predicts. Read at session start; append when you hit one.

- **Fix at the source when you can.** A gotcha is a bug report on our tooling, not a permanent fact — if it can be fixed in code, file it in `job` and fix it instead of recording it here.
- **Delete anything that becomes obvious, gets fixed, or stops recurring.** Keep this list short; a long list is one nobody reads.
- **Feedback about `AGENTS.md` itself** — a rule that was wrong, misread, or cost time — goes here too, prefixed `rule:`. It is harvested when the shared rules are reviewed.

Format: one dated H2 headline, then one paragraph. If an entry needs more than that, it's a finding — write it as a dated doc under `project/` and link it from the paragraph.

---

## 2026-11-06 — git in the Bash sandbox needs `GIT_CONFIG_GLOBAL=/dev/null`

The sandbox can't read `~/.gitconfig`, so `git clone` and friends fail with "unable to access '/Users/ben/.gitconfig': Operation not permitted". Prefix git commands with `GIT_CONFIG_GLOBAL=/dev/null`; the warning about `~/.config/git/ignore` is harmless.

## 2026-11-06 — subagents can't write report files

The harness refuses a subagent's Write to a findings file ("Subagents should return findings as text"). Brief research agents to return findings in their final message, and have the main thread save them.

## 2026-11-06 — `$TMPDIR` differs inside and outside the sandbox

Git writes to `.git` need the sandbox off, but an unsandboxed command sees a different `$TMPDIR` than the sandboxed one that wrote your commit-message file, so `-F $TMPDIR/msg.txt` fails. Write the message to `.git/<name>.txt` instead (untracked, writable once the sandbox is off), commit with `-F` on it, then delete it.
