; aicross 安装器钩子：安装时把安装目录加进当前用户的 PATH，卸载时移除这一项。
; 由 tauri.conf.json 的 bundle.windows.nsis.installerHooks 引入。
;
; 规则写在 path-entry.ps1，编进安装包，运行时放到临时插件目录再执行。不用
; -EncodedCommand：NSIS 单条字符串上限 1024 字符，编码后的脚本会被截断。
; 安装目录和操作经环境变量传入，路径里的引号影响不到脚本。脚本改了 PATH 时
; 退出 10，这时广播环境变量变更，新开的命令行即可用 aicross。

; 在文件顶层取本文件所在目录；宏展开时 __FILEDIR__ 指向的是 installer.nsi。
!define AICROSS_HOOKS_DIR "${__FILEDIR__}"

!macro AICROSS_PATH_ENTRY MODE
  InitPluginsDir
  File "/oname=$PLUGINSDIR\aicross-path-entry.ps1" "${AICROSS_HOOKS_DIR}\path-entry.ps1"
  System::Call 'Kernel32::SetEnvironmentVariable(t "AICROSS_PATH_MODE", t "${MODE}")'
  System::Call 'Kernel32::SetEnvironmentVariable(t "AICROSS_INSTALL_DIR", t "$INSTDIR")'
  nsExec::ExecToStack 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\aicross-path-entry.ps1"'
  Pop $R9
  Pop $R8
  System::Call 'Kernel32::SetEnvironmentVariable(t "AICROSS_PATH_MODE", p 0)'
  System::Call 'Kernel32::SetEnvironmentVariable(t "AICROSS_INSTALL_DIR", p 0)'
  ${If} $R9 == 10
    ; 0xFFFF = HWND_BROADCAST，0x001A = WM_SETTINGCHANGE
    SendMessage 0xFFFF 0x001A 0 "STR:Environment" /TIMEOUT=5000
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro AICROSS_PATH_ENTRY "add"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro AICROSS_PATH_ENTRY "remove"
!macroend