# CI consumer checks

Run `just quality` for non-mutating repository checks and
`.github/scripts/check-features.sh` for the isolated consumer checks. CI uses
project-maintained rumdl, typos and cargo-deny Actions, checksum-verified Taplo
and ShellCheck installation, and the official actionlint image. Local checks
use the same pinned tool versions specified in the workflow. CI selects latest
stable Rust explicitly while preserving the repository's local toolchain file.

`feature-consumer` declares its own workspace and lockfile. Each feature case
runs separately with `--no-default-features --locked`, so workspace tests and
other cases cannot enable missing optional dependencies. The consumer programs
exercise public APIs and assertions rather than inspecting workflow text.

The cases cover MCP with no features, its default derive re-export, and
separate chrono and rust_decimal integrations without derives. They check the
public schema output for primitive, derived, date/time, and Decimal types.

After intentionally changing dependencies, update both the workspace lockfile
and `feature-consumer/Cargo.lock`, then run the consumer checks again. These
checks do not replace the existing workspace tests or their doctests. Dependabot
includes both workspaces in its monthly grouped Cargo updates, preserving the
existing major-only policy.

The site job uses Dioxus CLI 0.7.9, matching the existing deployment workflow
through the pinned shared helper, and invokes the existing book, llms.txt and
static-site generators. Locked metadata is checked before the build, and
changes to tracked Cargo lockfiles fail the final drift check. The pinned
helper does not expose a way to pass `--locked` into nested Dioxus commands, so
the preflight and drift checks cover that limitation. The job uses read-only
repository permissions and uploads no deployment artifacts.

The dependency gate checks advisories and sources. Its Git allowlist mirrors
the existing lockfile; it contains no advisory ignores or new license policy.
