# 安装时把安装目录作为独立一项加进当前用户的 PATH，卸载时只删这一项（由 nsis-hooks.nsh 调用）。
# 读写注册表原始值：不展开 %VAR%，不改值的类型；比较时忽略大小写和结尾的 。
# 改了 PATH 时退出 10。
$mode = $env:AICROSS_PATH_MODE
$dir = $env:AICROSS_INSTALL_DIR.TrimEnd('\')
$key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
$raw = $key.GetValue('Path', $null, 'DoNotExpandEnvironmentNames')
$kind = if ($null -eq $raw) { 'ExpandString' } else { $key.GetValueKind('Path') }
$items = @(if ($raw) { $raw -split ';' | Where-Object { $_ -ne '' } })
$kept = @($items | Where-Object { -not [string]::Equals($_.TrimEnd('\'), $dir, 'OrdinalIgnoreCase') })
if ($mode -eq 'add') {
  if ($kept.Count -ne $items.Count) { exit 0 }
  $next = $items + $dir
} else {
  if ($kept.Count -eq $items.Count) { exit 0 }
  $next = $kept
}
$key.SetValue('Path', ($next -join ';'), $kind)
exit 10
