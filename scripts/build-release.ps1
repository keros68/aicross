# 打发布包：签名安装包并生成更新清单 latest.json。
#
# 用法：pwsh scripts/build-release.ps1 -Notes <说明文件>
#
# 签名私钥在 ~/.tauri/aicross-updater.key，密码以当前 Windows 用户加密存在
# ~/.tauri/aicross-updater-password.xml；两者都不进仓库，丢了私钥后已装的版本收不到
# 后续更新。密码只经环境变量传给 tauri 构建，不打印。
#
# 产物在 target/release/bundle/：nsis/*-setup.exe 与 .sig、msi/*.msi 与 .sig，以及
# target/release/bundle/latest.json。发布时把 setup.exe、msi 与 latest.json 一起上传；
# 软件按 releases/latest/download/latest.json 查更新，所以 latest.json 必须挂在最新一版上。

param([Parameter(Mandatory = $true)][string]$Notes)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$keyPath = Join-Path $HOME '.tauri\aicross-updater.key'
$passwordPath = Join-Path $HOME '.tauri\aicross-updater-password.xml'
foreach ($path in @($keyPath, $passwordPath, $Notes)) {
  if (-not (Test-Path $path)) { throw "找不到 $path" }
}

$secure = Import-Clixml $passwordPath
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw $keyPath
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Net.NetworkCredential]::new('', $secure).Password
try {
  Push-Location $root
  npx --yes -p @tauri-apps/cli@2 tauri build
  if ($LASTEXITCODE -ne 0) { throw "tauri build 失败：$LASTEXITCODE" }
} finally {
  Pop-Location
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}

$version = (Get-Content -Raw (Join-Path $root 'tauri.conf.json') | ConvertFrom-Json).version
$setup = "aicross_${version}_x64-setup.exe"
$sigPath = Join-Path $root "target\release\bundle\nsis\$setup.sig"
if (-not (Test-Path $sigPath)) { throw "没有生成签名：$sigPath" }

$manifest = [ordered]@{
  version   = $version
  notes     = (Get-Content -Raw -Encoding utf8 $Notes).Trim()
  pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  platforms = [ordered]@{
    'windows-x86_64' = [ordered]@{
      signature = (Get-Content -Raw $sigPath).Trim()
      url       = "https://github.com/keros68/aicross/releases/download/v$version/$setup"
    }
  }
}
$out = Join-Path $root 'target\release\bundle\latest.json'
[IO.File]::WriteAllText($out, ($manifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding $false))
"已生成：$out"
