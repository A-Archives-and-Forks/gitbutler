# but CLI Instructions

These supplement `crates/AGENTS.md` for work under `crates/but/`.

## CLI I/O

- Route user-visible command output through `out: &mut OutputChannel`:
  `out.for_human()` for human text, `out.for_shell()` for shell-friendly output,
  and `out.for_json()` with `write_value(...)` for JSON.
- Do not read `std::io::stdin()` directly in command or business logic. For
  interactive input, gate with `out.can_prompt()` or use
  `out.prepare_for_terminal_input()`; for piped or machine input, accept
  `read: impl std::io::Read` so tests can inject data. Keep `stdin().lock()` at
  top-level CLI wiring.

## CLI Tests

- In `crates/but/tests/`, prefer `env.but(...).assert().success()/failure()`
  with `.stdout_eq(snapbox::str![...])` and
  `.stderr_eq(snapbox::str![...])`; use `[..]` or `...` wildcards for unstable
  portions instead of weakening the assertion.
- Update CLI snapshots with `SNAPSHOTS=overwrite cargo nextest run -p but`,
  scoped to a test name when possible. For colored terminal output, assert
  against `snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"]`
  and update with the same command.
- Use sandbox helpers instead of `std::process::Command::new("git")`:
  `env.invoke_bash(...)` for multi-line command sequences and
  `env.invoke_git("...")` for single Git commands. Do not rewrite existing
  `env.invoke_bash(...)` calls just to use `env.invoke_git(...)`.
- Avoid `env.but(...).output()` followed by direct stdout/stderr assertions;
  keep output checks in snapbox. In tests, use panicking assertions such as
  `assert!`, `assert_eq!`, or `assert_ne!` rather than `anyhow::ensure!`.

## CLI Skills

- After changing CLI commands or workflows, update `crates/but/skill/` so
  bundled agent skills stay current.
