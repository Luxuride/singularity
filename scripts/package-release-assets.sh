#!/usr/bin/env bash
set -euo pipefail

if test "$#" -ne 1; then
  echo "Usage: $0 <tag>" >&2
  exit 1
fi

TAG="$1"

mkdir -p release-packages

package_group() {
  local source_dir="$1"
  local output_prefix="$2"
  shift 2

  for pattern in "$@"; do
    while IFS= read -r file; do
      cp "$file" "release-packages/${output_prefix}__$(basename "$file")"
    done < <(find "$source_dir" -type f -name "$pattern")
  done
}

# macOS packages
package_group "release-assets/macos-aarch64" "singularity-macos-aarch64-dmg" "*.dmg" "*.dmg.sig"
package_group "release-assets/macos-aarch64" "singularity-macos-aarch64-app" "*.app.tar.gz" "*.app.tar.gz.sig"

# Linux packages
package_group "release-assets/linux-x64" "singularity-linux-x64-deb" "*.deb" "*.deb.sig"
package_group "release-assets/linux-x64" "singularity-linux-x64-rpm" "*.rpm" "*.rpm.sig"
package_group "release-assets/linux-x64" "singularity-linux-x64-appimage" "*.AppImage" "*.AppImage.sig" "*.AppImage.tar.gz" "*.AppImage.tar.gz.sig"

# Windows packages
package_group "release-assets/windows-x64" "singularity-windows-x64-msi" "*.msi"
package_group "release-assets/windows-x64" "singularity-windows-x64-exe" "*.exe" "*.exe.sig"

# Flatpak package
package_group "release-assets/linux-flatpak" "singularity-linux-flatpak" "*.flatpak"
