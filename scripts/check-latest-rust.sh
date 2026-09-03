#!/usr/bin/env bash
set -euo pipefail

manifest_url="https://static.rust-lang.org/dist/channel-rust-stable.toml"
pinned="$(
  awk -F'"' '/^[[:space:]]*channel[[:space:]]*=/ { print $2; exit }' rust-toolchain.toml
)"

if [[ ! "${pinned}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "rust-toolchain.toml must pin an exact stable version, for example 1.98.0." >&2
  exit 1
fi

manifest_file="$(mktemp)"
trap 'rm -f "${manifest_file}"' EXIT

curl --fail --silent --show-error --location \
  --proto '=https' --tlsv1.2 --retry 3 \
  --output "${manifest_file}" \
  "${manifest_url}"

latest="$(
  awk '
      $0 == "[pkg.rust]" { in_rust = 1; next }
      in_rust && $1 == "version" {
        gsub(/"/, "", $3)
        print $3
        exit
      }
    ' "${manifest_file}"
)"

if [[ ! "${latest}" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "Could not determine the latest stable Rust release from ${manifest_url}." >&2
  exit 1
fi

if [[ "${pinned}" != "${latest}" ]]; then
  echo "Rust toolchain is stale: repository=${pinned}, latest-stable=${latest}." >&2
  echo "Update rust-toolchain.toml, Cargo.toml rust-version, Dockerfile and CI together." >&2
  exit 1
fi

echo "Rust ${pinned} is the latest stable release."
