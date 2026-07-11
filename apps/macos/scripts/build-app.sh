#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
app_dir="$(cd "$script_dir/.." && pwd)"
repo_dir="$(cd "$app_dir/../.." && pwd)"
configuration="${CONFIGURATION:-release}"
output_dir="${OUTPUT_DIR:-$repo_dir/dist}"
app_bundle="$output_dir/Devclean.app"
swift_build_args=(--package-path "$app_dir" -c "$configuration")

if [[ -n "${HELPER_EXECUTABLE:-}" ]]; then
  helper="$HELPER_EXECUTABLE"
  if [[ "${UNIVERSAL:-0}" == "1" ]]; then
    swift_build_args+=(--arch arm64 --arch x86_64)
  fi
elif [[ "${UNIVERSAL:-0}" == "1" ]]; then
  if ! command -v rustup >/dev/null 2>&1; then
    echo "UNIVERSAL=1 requires rustup or a prebuilt HELPER_EXECUTABLE" >&2
    exit 1
  fi
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  cargo build --manifest-path "$repo_dir/Cargo.toml" --release --locked --target aarch64-apple-darwin
  cargo build --manifest-path "$repo_dir/Cargo.toml" --release --locked --target x86_64-apple-darwin
  mkdir -p "$repo_dir/target/universal-apple-darwin/release"
  lipo -create \
    "$repo_dir/target/aarch64-apple-darwin/release/devclean" \
    "$repo_dir/target/x86_64-apple-darwin/release/devclean" \
    -output "$repo_dir/target/universal-apple-darwin/release/devclean"
  helper="$repo_dir/target/universal-apple-darwin/release/devclean"
  swift_build_args+=(--arch arm64 --arch x86_64)
else
  cargo build --manifest-path "$repo_dir/Cargo.toml" --release --locked
  helper="$repo_dir/target/release/devclean"
fi

swift build "${swift_build_args[@]}"
swift_bin_dir="$(swift build "${swift_build_args[@]}" --show-bin-path)"

rm -rf "$app_bundle"
mkdir -p "$app_bundle/Contents/MacOS" "$app_bundle/Contents/Helpers"
cp "$swift_bin_dir/DevcleanMenuBar" "$app_bundle/Contents/MacOS/DevcleanMenuBar"
cp "$helper" "$app_bundle/Contents/Helpers/devclean"
cp "$app_dir/Resources/Info.plist" "$app_bundle/Contents/Info.plist"
chmod 755 "$app_bundle/Contents/MacOS/DevcleanMenuBar" "$app_bundle/Contents/Helpers/devclean"

codesign --force --deep --sign "${CODE_SIGN_IDENTITY:--}" "$app_bundle"
codesign --verify --deep --strict --verbose=2 "$app_bundle"

echo "$app_bundle"
