; Kosmos Desktop installer (Engine + GPUI components) — no Electron.
;
; Usage:
;   makensis.exe /DVERSION=1.2.3 /DSTAGE_DIR=C:\...\installer-stage /DOUT_FILE=C:\...\Kosmos-Setup-1.2.3.exe installer.nsi

!ifndef VERSION
  !error "VERSION must be defined via /DVERSION=<semver>"
!endif
!ifndef STAGE_DIR
  !error "STAGE_DIR must be defined via /DSTAGE_DIR=<path>"
!endif
!ifndef OUT_FILE
  !error "OUT_FILE must be defined via /DOUT_FILE=<path>"
!endif

!define APP_NAME "Kosmos"
!define APP_ID "com.kazui.kosmos"
!define PUBLISHER "Kazui"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Kosmos"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"

Name "${APP_NAME} ${VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Kosmos"
RequestExecutionLevel user
ShowInstDetails show
ShowUninstDetails show

Icon "${STAGE_DIR}\resources\icon.ico"
UninstallIcon "${STAGE_DIR}\resources\icon.ico"

Page directory
Page instfiles

UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetShellVarContext current

  ; Upgrade from an Electron install: stop the old shell and Engine, remove
  ; the old Electron files under $INSTDIR, keep all user data elsewhere.
  nsExec::ExecToLog 'taskkill /F /IM Kosmos.exe'
  nsExec::ExecToLog 'taskkill /F /IM kepler-backend.exe'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Manager.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Agenda.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Memoria.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Dictation.exe"'
  Sleep 500

  ; Replace the application payload only. User data lives in
  ; %APPDATA%\Kosmos and %LOCALAPPDATA%\Kosmos — never touched here.
  RMDir /r "$INSTDIR"
  CreateDirectory "$INSTDIR"
  File /r "${STAGE_DIR}\*"

  ; KOS-233: install the Engine bundled with this build. The script is
  ; monotonic and refuses to downgrade a newer Engine left by a later
  ; Desktop version.
  StrCpy $R0 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  nsExec::ExecToStack '"$R0" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\resources\install-engine.ps1" -Archive "$INSTDIR\resources\Kosmos Engine.zip" -Manifest "$INSTDIR\resources\engine-manifest.json" -TargetRoot "$LOCALAPPDATA\Kosmos\Engine"'
  Pop $0
  Pop $1
  StrCmp $0 "0" engine_ready
    Abort "Kosmos Engine installation failed: $1"
  engine_ready:

  ; Windows entry points open the GPUI Manager.
  Delete "$DESKTOP\CosCast.lnk"
  Delete "$SMPROGRAMS\CosCast.lnk"
  Delete "$DESKTOP\Kosmos.lnk"
  Delete "$SMPROGRAMS\Kosmos.lnk"
  CreateShortCut "$SMPROGRAMS\Kosmos.lnk" "$INSTDIR\resources\components\manager\Kosmos Manager.exe" "" "$INSTDIR\resources\icon.ico" 0

  ; Optional GPUI components get their own Start Menu shortcuts.
  IfFileExists "$INSTDIR\resources\components\agenda\Kosmos Agenda.exe" 0 agenda_shortcut_done
    CreateShortCut "$SMPROGRAMS\Kosmos Agenda.lnk" "$INSTDIR\resources\components\agenda\Kosmos Agenda.exe" "" "$INSTDIR\resources\icon.ico" 0
  agenda_shortcut_done:
  IfFileExists "$INSTDIR\resources\components\memoria\Kosmos Memoria.exe" 0 memoria_shortcut_done
    CreateShortCut "$SMPROGRAMS\Kosmos Memoria.lnk" "$INSTDIR\resources\components\memoria\Kosmos Memoria.exe" "" "$INSTDIR\resources\icon.ico" 0
  memoria_shortcut_done:
  IfFileExists "$INSTDIR\resources\components\dictation\Kosmos Dictation.exe" 0 dictation_shortcut_done
    CreateShortCut "$SMPROGRAMS\Kosmos Dictation.lnk" "$INSTDIR\resources\components\dictation\Kosmos Dictation.exe" "" "$INSTDIR\resources\icon.ico" 0
  dictation_shortcut_done:

  ; Kepler → Kosmos product rename.
  Delete "$DESKTOP\Kepler.lnk"
  Delete "$SMPROGRAMS\Kepler.lnk"

  ; Autostart = Engine. The versioned path is maintained by
  ; install-engine.ps1; seed it here so first-run installs autostart too.
  WriteRegStr HKCU "${RUN_KEY}" "Kosmos Engine" '"$LOCALAPPDATA\Kosmos\Engine\versions\${VERSION}\kepler-backend.exe" --start'

  ; Uninstall registration.
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\resources\icon.ico"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
SectionEnd

Section "Uninstall"
  SetShellVarContext current

  Delete "$DESKTOP\Kosmos.lnk"
  Delete "$SMPROGRAMS\Kosmos.lnk"
  Delete "$SMPROGRAMS\Kosmos Agenda.lnk"
  Delete "$SMPROGRAMS\Kosmos Memoria.lnk"
  Delete "$SMPROGRAMS\Kosmos Dictation.lnk"
  Delete "$DESKTOP\Kepler.lnk"
  Delete "$SMPROGRAMS\Kepler.lnk"
  Delete "$DESKTOP\CosCast.lnk"
  Delete "$SMPROGRAMS\CosCast.lnk"

  DeleteRegValue HKCU "${RUN_KEY}" "Kosmos Engine"
  DeleteRegValue HKCU "${RUN_KEY}" "Kosmos"
  DeleteRegValue HKCU "${RUN_KEY}" "com.kazui.kepler"
  DeleteRegValue HKCU "${RUN_KEY}" "Kepler"
  DeleteRegValue HKCU "${RUN_KEY}" "KeplerKosmos"
  DeleteRegValue HKCU "${RUN_KEY}" "KosmosKepler"

  ; Remove the application payload; user data in %APPDATA%\Kosmos and
  ; %LOCALAPPDATA%\Kosmos (including the Engine) is intentionally kept.
  RMDir /r "$INSTDIR"

  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
