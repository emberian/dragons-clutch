# Supervised Kimi review lane

This directory makes Kimi useful for narrow code-reading lanes without giving
the model a shell, file writes, web access, or subagents. It is **capability
reduction, not a security sandbox**: Kimi Code 0.36.1 permits its file tools to
read an explicitly absolute path outside the workspace, and its path guard is
lexical rather than symlink-resolving. The reviewer prompt forbids both, but a
supervisor must still keep secrets out of the task and inspect the result.

## Recommended structured workflow

From the repository root:

```sh
tools/kimi/review new \
  "Inspect only path/to/file.rs. Review one named invariant; cite lines."
```

The command prints a session ID and a private artifact directory under
`.git/kimi-runs/`. Continue the same conversation with:

```sh
tools/kimi/review resume session_... \
  "Follow up on the first finding; do not inspect additional paths."
```

The wrapper verifies that a resumed session belongs to this checkout and was
bound to the exact read-only profile. Each run records:

- `events.jsonl`: Kimi's machine-readable Assistant/tool event stream;
- `final.md`: the last Assistant message extracted from that stream;
- `stderr.log`: progress and diagnostics;
- `provenance.json`: CLI/model/profile digests, prompt digest, session ID, exit
  status, output digests, and whether Git HEAD/status changed during the run.

The Git comparison is an observation, not attribution: another agent may edit or
commit concurrently. A change therefore produces a warning but does not make an
otherwise successful review fail. The enforced protection is the profile's
three-tool allowlist, not the status digest.

Artifacts are mode 0600 and live under `.git`; do not publish them without
review. Tool results can contain substantial source text. Kimi separately
persists the full prompt and conversation below `~/.kimi-code/sessions/` (or
`$KIMI_CODE_HOME/sessions/`).

## Interactive conversation

For a human-supervised TUI session:

```sh
cd /Users/ember/dev/dragons-clutch
kimi --model=kimi-code/k3 \
  --agent-file=/Users/ember/dev/dragons-clutch/tools/kimi/read-only-reviewer.md
```

Use `/status` to inspect the runtime and `/title` to give the session a useful
name. Exit normally, then resume exactly that conversation with:

```sh
cd /Users/ember/dev/dragons-clutch
kimi --session=session_...
```

Do not pass `--agent-file` again on resume: the profile is bound at session
creation and restored with the session.

## Direct structured commands

The wrapper is preferred. The equivalent raw new-session command is:

```sh
kimi \
  --model=kimi-code/k3 \
  --agent-file=/Users/ember/dev/dragons-clutch/tools/kimi/read-only-reviewer.md \
  --output-format=stream-json \
  --prompt="Inspect only ..."
```

Resume it non-interactively with:

```sh
kimi \
  --session=session_... \
  --output-format=stream-json \
  --prompt="Follow up ..."
```

Use the long `--option=value` forms shown here. With Kimi Code 0.36.1, the
natural-looking mixed form `kimi -p --agent-file path ... "prompt"` was observed
to misparse `path` as a subcommand and fail with `unknown command`.

## Safety and fitness

- `--prompt` mode uses Kimi's automatic permission policy and never asks for
  approval. It is acceptable here only because the selected profile exposes
  `Read`, `Grep`, and `Glob` and nothing else. Never substitute the default agent
  for unattended prompt mode.
- Do not use `--yolo` or `--auto` for this lane.
- The current tested stack is Kimi Code CLI 0.36.1 with model alias
  `kimi-code/k3` (provider model `k3`). Authentication is the user's existing
  managed Kimi OAuth configuration; the model call necessarily leaves the
  machine.
- A working directory is context, not a hard read boundary. Do not give this
  lane secrets or use it on an untrusted tree containing outward-pointing
  symlinks.
- This lane is suitable for supervised code/document review, bounded inventory,
  and a second opinion. It is not suitable for autonomous edits, shell work,
  release/deployment actions, formal-verification claims, or final security and
  financial sign-off.
- `kimi doctor` currently reports a harmless local configuration warning:
  `max_retries_per_step` is deprecated in favor of `max_attempts_per_step`.
