## Why

<!-- One short paragraph or a few bullets on what's broken or missing today. -->

## What

<!-- Concrete change list. Reference filenames / modules where helpful. -->

## Tests

<!--
cargo fmt --all --check                                            -> clean
cargo clippy --locked --workspace --all-targets -- -D warnings     -> clean
cargo test --locked --workspace                                    -> N passed
cargo run --locked -p cli -- doctor --config examples/minimal.toml -> clean
-->

<!--
Optional sections, add when relevant:

## Compatibility
- When manifest shape, CLI output keys, protocol fields, or exit-code semantics move.

## Trade-off worth flagging
- A known downside reviewers should hold in mind.

See "Standard Workflow" in AGENTS.md for the full shape.
-->
