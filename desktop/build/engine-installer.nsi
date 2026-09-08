Unicode true
!include "MUI2.nsh"
Name "Kosmos Engine"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Kosmos\Engine"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "Kosmos Engine"
VIAddVersionKey "FileDescription" "Kosmos Engine Installer"
VIAddVersionKey "FileVersion" "${VERSION}"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Russian"

Function .onInit
  ; Engine owns one per-user installation, independent of the Desktop directory.
  StrCpy $INSTDIR "$LOCALAPPDATA\Kosmos\Engine"
FunctionEnd

Section "Engine"
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=engine.zip "${PAYLOAD}\Kosmos-Engine.zip"
  File /oname=manifest.json "${PAYLOAD}\engine-manifest.json"
  File "install-engine.ps1"
  StrCpy $R0 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  nsExec::ExecToStack '"$R0" -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\install-engine.ps1" -Archive "$PLUGINSDIR\engine.zip" -Manifest "$PLUGINSDIR\manifest.json" -TargetRoot "$INSTDIR"'
  Pop $0
  Pop $1
  StrCmp $0 "0" +3
    SetErrorLevel 1
    Abort "Не удалось установить Kosmos Engine: $1"
  SetOutPath "$INSTDIR"
  File "install-engine.ps1"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "DisplayName" "Kosmos Engine"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "Publisher" "Kosmos"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "DisplayIcon" "$INSTDIR\versions\${VERSION}\kepler-backend.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "NoRepair" 1
  CreateShortCut "$SMPROGRAMS\Kosmos Engine.lnk" "$INSTDIR\versions\${VERSION}\kepler-backend.exe" "--start"
SectionEnd

Section "Uninstall"
  ; Remove only this installer's known files. User data in Roaming is retained.
  ; A locked file aborts before registration is changed, so Apps & Features
  ; never claims success while the installed Engine is still present.
  StrCpy $R0 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  nsExec::ExecToStack '"$R0" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\install-engine.ps1" -Uninstall -Version "${VERSION}" -TargetRoot "$INSTDIR"'
  Pop $0
  Pop $1
  StrCmp $0 "0" uninstall_current
  StrCmp $0 "2" uninstall_old_version
  Abort "Не удалось удалить Kosmos Engine: один из файлов занят. Закройте Engine и повторите удаление."
  uninstall_current:
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos Engine"
  Delete "$SMPROGRAMS\Kosmos Engine.lnk"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine"
  Delete "$INSTDIR\install-engine.ps1"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR\versions"
  RMDir "$INSTDIR"
  Goto uninstall_done
  uninstall_old_version:
  ; Another Engine version owns the pointer and registration; keep its entry points.
  uninstall_done:
SectionEnd
