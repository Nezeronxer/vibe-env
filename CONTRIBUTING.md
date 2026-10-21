# Contributing

Thanks for looking. Keep changes small and tested.

## Set up

Requirements: stable Rust (cargo 1.96 or newer), git, and on Linux the Tauri 2 libraries listed in the
README. No Node toolchain: `ui/` is plain HTML/CSS/JS without a bundler.

```sh
cargo test -p nightshift-core     # core tests
cargo build -p nightshift-app     # the window app (also validates app/tauri.conf.json)
node scripts/ui-selftest.js       # UI logic checks, no window needed
cd app && cargo tauri dev         # run the app
```

## Rules

- `core` is a library without Tauri. Do not add dependencies without a reason in the PR text.
- External processes go through `std::process::Command`; no `sh -c` with pasted data.
- Identifiers and comments are English. Strings for people live in the dictionaries
  `ui/i18n.js` and `ui/i18n-v2.js` (ru and en); core reports short English keys, the UI translates.
- No secrets, no personal data, no telemetry, no own server endpoints.
- A new agent environment is an adapter in `core/src/agent/`; mark every flag you could not run
  yourself as unverified (see how existing adapters do it).
- Vendor logos: add the file to `ui/icons/` and its source and terms to `docs/icons-sources.md`.
  SVG must have no scripts, no external links, no `foreignObject`.

## Pull requests

One topic per PR, tests for new logic, `cargo test --workspace` green. By contributing you agree your
work is released under the MIT license (see `LICENSE`).
