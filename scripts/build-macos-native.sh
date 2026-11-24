#!/usr/bin/env bash
# 打 macOS（Apple Silicon）原生 SwiftUI（Liquid Glass）安装包：aicross.app 与 .dmg。
#
# 用法：scripts/build-macos-native.sh
#
# 流程：
#   1. 以 `--no-default-features` 编译轻量无头 Rust 引擎并置入 macos/AICross/Resources/aicross-engine；
#   2. 调用 Xcode 编译原生 SwiftUI 应用（macos/AICross.xcodeproj）；
#   3. 本地签名并生成 releases/aicross_<version>_aarch64.dmg。

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "只支持 Apple Silicon 的 macOS。" >&2
  exit 1
fi

if [ -z "${DEVELOPER_DIR:-}" ] && [ -d "/Applications/Xcode.app/Contents/Developer" ]; then
  export DEVELOPER_DIR="/Applications/Xcode.app/Contents/Developer"
fi

version="$(python3 -c 'import re,pathlib; m=re.search(r"^version\s*=\s*\"([^\"]+)\"", pathlib.Path("Cargo.toml").read_text(), re.M); print(m.group(1))')"

echo "1/3 编译无头 Rust 引擎（v${version}）…"
cargo build --release --no-default-features
mkdir -p "$root/macos/AICross/Resources"
cp "$root/target/release/aicross" "$root/macos/AICross/Resources/aicross-engine"
chmod +x "$root/macos/AICross/Resources/aicross-engine"

echo "2/3 编译原生 macOS SwiftUI（Liquid Glass）面板…"
xcodebuild \
  -project "$root/macos/AICross.xcodeproj" \
  -scheme AICross \
  -configuration Release \
  -derivedDataPath "$root/macos/build" \
  build >/dev/null

app_path="$root/macos/build/Build/Products/Release/aicross.app"
xattr -cr "$app_path" 2>/dev/null || true
codesign --force --deep --sign - "$app_path"
codesign --verify --deep --strict "$app_path"

echo "3/3 打包 DMG 安装镜像…"
mkdir -p "$root/releases"
dmg_path="$root/releases/aicross_${version}_aarch64.dmg"
stage_dir="$(mktemp -d /tmp/aicross-native-dmg.XXXXXX)"
trap 'rm -rf "$stage_dir"' EXIT

ditto "$app_path" "$stage_dir/aicross.app"
ln -s /Applications "$stage_dir/Applications"
rm -f "$dmg_path"
hdiutil create -volname "aicross" -srcfolder "$stage_dir" -ov -format UDZO "$dmg_path" >/dev/null

echo "已生成：$dmg_path"
shasum -a 256 "$dmg_path"
