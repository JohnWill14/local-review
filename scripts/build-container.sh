#!/usr/bin/env sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
mkdir -p "$project_dir/release"

podman build -t local-review-builder -f "$project_dir/container/Containerfile" "$project_dir"
podman run --rm \
  -v "$project_dir:/workspace:ro,Z" \
  -v local-review-cargo-target:/workspace/target:Z \
  -v local-review-cargo-registry:/usr/local/cargo/registry:Z \
  -v local-review-cargo-git:/usr/local/cargo/git:Z \
  -v "$project_dir/release:/out:Z" \
  -w /workspace \
  local-review-builder \
  sh -c 'cargo build --release --locked && cp target/release/local-review /out/local-review.new && mv -f /out/local-review.new /out/local-review'

printf '%s\n' "Ready: $project_dir/release/local-review"
