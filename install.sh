#!/bin/sh
# Installs the latest terminal-scheme-info release for this machine (Linux, macOS).
#
#   curl -fsSL https://raw.githubusercontent.com/mopi1402/terminal-scheme-info/main/install.sh | sh
#
# TSI_VERSION      version to install (default: the latest release)
# TSI_INSTALL_DIR  where to put the binary (default: ~/.local/bin, /usr/local/bin as root)
set -eu
repo=mopi1402/terminal-scheme-info
name=terminal-scheme-info

os=$(uname -s)
arch=$(uname -m)
case "$os:$arch" in
    Darwin:arm64) target=aarch64-apple-darwin ;;
    Darwin:x86_64) target=x86_64-apple-darwin ;;
    Linux:aarch64 | Linux:arm64) target=aarch64-unknown-linux-musl ;;
    Linux:x86_64 | Linux:amd64) target=x86_64-unknown-linux-musl ;;
    *) echo "$name: no prebuilt binary for $os $arch; see https://github.com/$repo/releases" >&2; exit 1 ;;
esac

fetch() {
    if command -v curl >/dev/null 2>&1; then curl -fsSL "$1"; else wget -qO- "$1"; fi
}

version=${TSI_VERSION:-}
if [ -z "$version" ]; then
    version=$(fetch "https://api.github.com/repos/$repo/releases/latest" | sed -n 's/.*"tag_name": *"v\([^"]*\)".*/\1/p' | head -1)
    [ -n "$version" ] || { echo "$name: cannot determine the latest version" >&2; exit 1; }
fi

base="https://github.com/$repo/releases/download/v$version"
archive="$name-$version-$target.tar.gz"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading $archive"
fetch "$base/$archive" > "$tmp/$archive"
fetch "$base/SHA256SUMS" > "$tmp/SHA256SUMS"
(
    cd "$tmp"
    grep " $archive\$" SHA256SUMS > expected
    if command -v sha256sum >/dev/null 2>&1; then sha256sum -c expected; else shasum -a 256 -c expected; fi
) > /dev/null || { echo "$name: checksum mismatch, aborting" >&2; exit 1; }

tar xzf "$tmp/$archive" -C "$tmp"
if [ -n "${TSI_INSTALL_DIR:-}" ]; then dir=$TSI_INSTALL_DIR
elif [ "$(id -u)" = 0 ]; then dir=/usr/local/bin
else dir=$HOME/.local/bin
fi
mkdir -p "$dir"
install -m 755 "$tmp/$name-$version-$target/$name" "$dir/$name"
echo "installed $name $version to $dir/$name"
case ":$PATH:" in
    *":$dir:"*) ;;
    *) echo "note: $dir is not on your PATH" ;;
esac
echo "next: $dir/$name install    # adds the line to your shell startup file"
