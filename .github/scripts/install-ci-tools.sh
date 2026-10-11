#!/usr/bin/env bash
set -euo pipefail

# Keep the local formatter versions and their CI flags in sync.
cargo binstall taplo-cli --version 0.10.0 --locked --no-confirm
cargo binstall rumdl --version 0.2.78 --locked --no-confirm
cargo binstall cargo-sort-derives --version 0.13.0 --locked --no-confirm
cargo binstall cargo-deny --version 0.20.2 --locked --no-confirm

install_dir="${RUNNER_TEMP:?}/ci-tools"
archive_dir="$(mktemp -d)"
trap 'rm -rf "$archive_dir"' EXIT
mkdir -p "$install_dir"
fetch_archive() {
    local url="$1" digest="$2" archive="$3"
    curl --fail --location --retry 3 "$url" --output "$archive_dir/$archive"
    printf '%s  %s\n' "$digest" "$archive_dir/$archive" | sha256sum --check
}
fetch_archive https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz 8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8 actionlint.tar.gz
tar -xzf "$archive_dir/actionlint.tar.gz" -C "$install_dir" actionlint
fetch_archive https://github.com/koalaman/shellcheck/releases/download/v0.11.0/shellcheck-v0.11.0.linux.x86_64.tar.xz 8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198 shellcheck.tar.xz
tar -xJf "$archive_dir/shellcheck.tar.xz" -C "$archive_dir"
cp "$archive_dir/shellcheck-v0.11.0/shellcheck" "$install_dir/"
fetch_archive https://github.com/crate-ci/typos/releases/download/v1.42.3/typos-v1.42.3-x86_64-unknown-linux-musl.tar.gz e083bf917d840563e77b35b64f43924d98fcb77acb69691455099d39e347d67d typos.tar.gz
tar -xzf "$archive_dir/typos.tar.gz" -C "$install_dir" typos
printf '%s\n' "$install_dir" >> "${GITHUB_PATH:?}"
