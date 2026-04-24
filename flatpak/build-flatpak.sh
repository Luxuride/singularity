#!/bin/sh
set -eu

mkdir -p npm-cache
cp -a flatpak-node/npm-cache/. npm-cache/

mkdir -p .cargo
cat > .cargo/config.toml <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "cargo/vendor"

[patch.crates-io]
matrix-sdk = { path = "vendor-patches/matrix-sdk" }
EOF

mkdir -p vendor-patches
rm -rf vendor-patches/matrix-sdk
cp -a cargo/vendor/matrix-sdk-0.16.0 vendor-patches/matrix-sdk
if ! grep -Fq '#![recursion_limit = "256"]' vendor-patches/matrix-sdk/src/lib.rs; then
  sed -i '1i #![recursion_limit = "256"]' vendor-patches/matrix-sdk/src/lib.rs
fi

npm ci --offline
npm run tauri -- build --no-bundle --config flatpak/tauri.flatpak.conf.json -- --offline