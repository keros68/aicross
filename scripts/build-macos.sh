#!/usr/bin/env bash
# 打 macOS（Apple Silicon）安装包：.app 与 .dmg，不做 Developer ID 签名与公证。
#
# 用法：scripts/build-macos.sh [-n 说明文件]
#
# 没有更新签名私钥时只出 .dmg，不生成更新包。私钥在 ~/.tauri/aicross-updater.key，
# 密码经环境变量 TAURI_SIGNING_PRIVATE_KEY_PASSWORD 提供；两者都有时另外生成签名的
# 更新包 aicross.app.tar.gz，并把 darwin-aarch64 一项并进 latest.json（与 Windows 机器
# 上 build-release.ps1 生成的 windows-x86_64 一项放在同一份文件里）。密钥与密码不打印。
#
# 产物在 target/aarch64-apple-darwin/release/bundle/：macos/aicross.app、dmg/*.dmg；
# 有私钥时还有 macos/aicross.app.tar.gz 与 .sig，以及 target/release/bundle/latest.json。
# 该位置已有 latest.json 时（先把 Windows 机器上生成的拷过来）保留原有平台，只更新
# darwin-aarch64。发布时把 aicross_aarch64.app.tar.gz 与 latest.json 一起上传。

set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

notes_file=""
while getopts "n:" opt; do
  case "$opt" in
    n) notes_file="$OPTARG" ;;
    *) echo "用法：$0 [-n 说明文件]" >&2; exit 2 ;;
  esac
done

if [ "$(uname -s)" != "Darwin" ] || [ "$(uname -m)" != "arm64" ]; then
  echo "只支持 Apple Silicon 的 macOS。" >&2
  exit 1
fi

key_path="$HOME/.tauri/aicross-updater.key"
out="$root/target/aarch64-apple-darwin/release/bundle"
bundle_dir="$root/target/release/bundle"
version="$(node -p "require('./tauri.conf.json').version")"

sign=0
if [ -f "$key_path" ] && [ -n "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]; then
  sign=1
fi

if [ "$sign" = 1 ]; then
  TAURI_SIGNING_PRIVATE_KEY="$(cat "$key_path")"
  export TAURI_SIGNING_PRIVATE_KEY
  trap 'unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD' EXIT
  npx --yes -p @tauri-apps/cli@2 tauri build --target aarch64-apple-darwin --bundles app,dmg
else
  echo "没有更新签名私钥或密码，只生成 .dmg，不生成更新包。"
  npx --yes -p @tauri-apps/cli@2 tauri build --target aarch64-apple-darwin --bundles app,dmg \
    --config '{"bundle":{"createUpdaterArtifacts":false}}'
fi

dmg="$(ls "$out"/dmg/*.dmg | head -n 1)"
echo "已生成：$dmg"

if [ "$sign" = 0 ]; then
  exit 0
fi

archive="$out/macos/aicross.app.tar.gz"
sig_path="$archive.sig"
[ -f "$sig_path" ] || { echo "没有生成签名：$sig_path" >&2; exit 1; }
notes=""
[ -n "$notes_file" ] && notes="$(cat "$notes_file")"

mkdir -p "$bundle_dir"
VERSION="$version" NOTES="$notes" SIG_PATH="$sig_path" LATEST="$bundle_dir/latest.json" node -e '
const fs = require("fs");
const { VERSION, NOTES, SIG_PATH, LATEST } = process.env;
let manifest = {};
if (fs.existsSync(LATEST)) manifest = JSON.parse(fs.readFileSync(LATEST, "utf8"));
const platforms = manifest.platforms || {};
platforms["darwin-aarch64"] = {
  signature: fs.readFileSync(SIG_PATH, "utf8").trim(),
  url: `https://github.com/keros68/aicross/releases/download/v${VERSION}/aicross_aarch64.app.tar.gz`,
};
manifest = {
  version: VERSION,
  notes: NOTES.trim() || manifest.notes || "",
  pub_date: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  platforms,
};
fs.writeFileSync(LATEST, JSON.stringify(manifest, null, 2));
'
cp "$archive" "$bundle_dir/aicross_aarch64.app.tar.gz"
echo "已生成：$bundle_dir/latest.json"
