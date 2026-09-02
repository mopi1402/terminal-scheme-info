#!/bin/sh
# Writes the Homebrew formula and the Scoop manifest for one release, from its
# SHA256SUMS file. Used by the release workflow to update the tap and bucket.
#
#   scripts/package-manifests.sh VERSION SHA256SUMS OUTDIR
set -eu
version=$1 sums=$2 out=$3
repo=mopi1402/terminal-scheme-info
name=terminal-scheme-info
base="https://github.com/$repo/releases/download/v$version"
desc="Expose the terminal's background, foreground and colour scheme as environment variables"
mkdir -p "$out"

hash() { # hash TARGET EXT
    grep " $name-$version-$1.$2\$" "$sums" | cut -d' ' -f1
}

cat > "$out/$name.rb" <<RUBY
class TerminalSchemeInfo < Formula
  desc "$desc"
  homepage "https://github.com/$repo"
  version "$version"
  license "MIT"

  on_macos do
    on_arm do
      url "$base/$name-$version-aarch64-apple-darwin.tar.gz"
      sha256 "$(hash aarch64-apple-darwin tar.gz)"
    end
    on_intel do
      url "$base/$name-$version-x86_64-apple-darwin.tar.gz"
      sha256 "$(hash x86_64-apple-darwin tar.gz)"
    end
  end

  on_linux do
    on_arm do
      url "$base/$name-$version-aarch64-unknown-linux-musl.tar.gz"
      sha256 "$(hash aarch64-unknown-linux-musl tar.gz)"
    end
    on_intel do
      url "$base/$name-$version-x86_64-unknown-linux-musl.tar.gz"
      sha256 "$(hash x86_64-unknown-linux-musl tar.gz)"
    end
  end

  def install
    bin.install "$name"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/$name --version")
  end
end
RUBY

cat > "$out/$name.json" <<JSON
{
    "version": "$version",
    "description": "$desc",
    "homepage": "https://github.com/$repo",
    "license": "MIT",
    "architecture": {
        "64bit": {
            "url": "$base/$name-$version-x86_64-pc-windows-msvc.zip",
            "hash": "$(hash x86_64-pc-windows-msvc zip)",
            "extract_dir": "$name-$version-x86_64-pc-windows-msvc"
        },
        "arm64": {
            "url": "$base/$name-$version-aarch64-pc-windows-msvc.zip",
            "hash": "$(hash aarch64-pc-windows-msvc zip)",
            "extract_dir": "$name-$version-aarch64-pc-windows-msvc"
        }
    },
    "bin": "$name.exe",
    "checkver": "github",
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/$repo/releases/download/v\$version/$name-\$version-x86_64-pc-windows-msvc.zip",
                "extract_dir": "$name-\$version-x86_64-pc-windows-msvc"
            },
            "arm64": {
                "url": "https://github.com/$repo/releases/download/v\$version/$name-\$version-aarch64-pc-windows-msvc.zip",
                "extract_dir": "$name-\$version-aarch64-pc-windows-msvc"
            }
        },
        "hash": {
            "url": "\$baseurl/SHA256SUMS"
        }
    }
}
JSON
echo "wrote $out/$name.rb and $out/$name.json"
