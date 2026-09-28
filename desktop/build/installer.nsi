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

; The install directory is fixed so the installer never recursively deletes a
; user-chosen location.
Page instfiles

UninstPage uninstConfirm
UninstPage instfiles

!macro KillKosmosProcesses
  nsExec::ExecToLog 'taskkill /F /IM Kosmos.exe'
  nsExec::ExecToLog 'taskkill /F /IM kepler-backend.exe'
  ; 0.9.x installs leave an orphaned ark-core-rpc.exe child holding the DB.
  nsExec::ExecToLog 'taskkill /F /IM ark-core-rpc.exe'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Manager.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Agenda.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Memoria.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Dictation.exe"'
  Sleep 500
!macroend

!macro ReadOldElectronAutostartDetected
  ReadRegStr $R0 HKCU "${RUN_KEY}" "com.kazui.kosmos"
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "electron.app.Kosmos"
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "Kosmos"
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "com.kazui.kepler"
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "Kepler"
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "KeplerKosmos"
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "KosmosKepler"
  StrCmp $R0 "" +2
    StrCpy $R2 1
!macroend

!macro DeleteOldElectronRunValues
  DeleteRegValue HKCU "${RUN_KEY}" "com.kazui.kosmos"
  DeleteRegValue HKCU "${RUN_KEY}" "electron.app.Kosmos"
  DeleteRegValue HKCU "${RUN_KEY}" "Kosmos"
  DeleteRegValue HKCU "${RUN_KEY}" "com.kazui.kepler"
  DeleteRegValue HKCU "${RUN_KEY}" "Kepler"
  DeleteRegValue HKCU "${RUN_KEY}" "KeplerKosmos"
  DeleteRegValue HKCU "${RUN_KEY}" "KosmosKepler"
!macroend

Function .onInit
  ; Legacy electron-updater passes arguments like --updated and --force-run.
  ; NSIS does not recognise them, so we simply ignore them rather than failing.
FunctionEnd

; Returns:
;   $R2 = 1 if an old Electron autostart Run value was present.
;   $R3 = 1 only if no previous Kosmos install of either kind was detected.
Function DetectPreviousKosmos
  StrCpy $R2 0
  StrCpy $R3 1

  ; Old Electron installation.
  IfFileExists "$LOCALAPPDATA\Programs\kepler-shell" 0 +2
    StrCpy $R3 0
  ; The Electron uninstall keys are the bare GUIDs — no braces.
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\4fe2b964-4d0e-5a72-8728-cca14468c9f0" "DisplayName"
  StrCmp $R0 "" +2
    StrCpy $R3 0
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d" "DisplayName"
  StrCmp $R0 "" +2
    StrCpy $R3 0

  ; Current Engine / Desktop installation.
  ReadRegStr $R0 HKCU "${UNINSTALL_KEY}" "DisplayName"
  StrCmp $R0 "" +2
    StrCpy $R3 0
  IfFileExists "$LOCALAPPDATA\Kosmos\Engine\current.json" 0 +2
    StrCpy $R3 0

  ; Any surviving old Electron autostart value counts as a previous install
  ; and, if present, means we should migrate the autostart preference.
  !insertmacro ReadOldElectronAutostartDetected
  IntCmp $R2 1 0 +2
    StrCpy $R3 0
FunctionEnd

; Seeds Engine autostart only on a fresh install, or migrates the old Electron
; autostart preference when upgrading from the Electron app. Uses the Engine
; version actually installed by install-engine.ps1, never the Desktop VERSION.
Function SeedOrMigrateAutostart
  ; Only act for fresh installs or Electron migrations with autostart enabled.
  IntCmp $R3 1 do_autostart
  IntCmp $R2 1 do_autostart
  Return

do_autostart:
  StrCpy $R5 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R5 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"

  ; Post-install logic lives in a shipped script: NSIS single-quoted strings
  ; cannot contain inline PowerShell safely.
  nsExec::ExecToLog '"$R5" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\resources\engine-post-install.ps1" -SeedAutostart'
FunctionEnd

; Always start the installed Engine at the end of the install. On interactive
; installs also open the Manager so the user lands in the app.
Function StartEngineAndManager
  StrCpy $R5 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R5 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"

  nsExec::ExecToLog '"$R5" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\resources\engine-post-install.ps1" -StartEngine'

  IfSilent manager_done
  Exec '"$INSTDIR\resources\components\manager\Kosmos Manager.exe"'
manager_done:
FunctionEnd

Section "Install"
  SetShellVarContext current

  ; Determine whether this is a fresh install or a migration/upgrade before we
  ; remove any state.
  Call DetectPreviousKosmos

  StrCpy $R0 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"

  ; Stop all Kosmos processes before touching files or registry.
  !insertmacro KillKosmosProcesses

  ; Upgrade from an Electron install: remove the old shell payload, its stale
  ; Apps & Features entries, and its autostart Run values. We never run the old
  ; uninstaller and never touch %APPDATA%\Kosmos or %LOCALAPPDATA%\Kosmos.
  ; Old Electron autostart values are stale regardless of whether the
  ; kepler-shell payload survived; remove them unconditionally now that
  ; DetectPreviousKosmos has read them for the migration decision.
  !insertmacro DeleteOldElectronRunValues
  IfFileExists "$LOCALAPPDATA\Programs\kepler-shell" 0 legacy_cleanup_done
    RMDir /r "$LOCALAPPDATA\Programs\kepler-shell"
    ; Bare GUID keys — the real entries have no braces.
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\4fe2b964-4d0e-5a72-8728-cca14468c9f0"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d"

    ; Old Electron shortcuts were named Kosmos.lnk or Kepler.lnk on Desktop and
    ; in the Start Menu and pointed into kepler-shell.
    Delete "$DESKTOP\Kosmos.lnk"
    Delete "$SMPROGRAMS\Kosmos.lnk"
    Delete "$DESKTOP\Kepler.lnk"
    Delete "$SMPROGRAMS\Kepler.lnk"
  legacy_cleanup_done:

  ; Replace the shipped application payload only. User data lives in
  ; %APPDATA%\Kosmos and %LOCALAPPDATA%\Kosmos and is never touched here.
  IfFileExists "$INSTDIR\resources\*.*" 0 +2
    RMDir /r "$INSTDIR\resources"
  Delete "$INSTDIR\Uninstall.exe"
  SetOutPath "$INSTDIR"
  CreateDirectory "$INSTDIR"
  File /r "${STAGE_DIR}\*"

  ; KOS-233: install the Engine bundled with this build. The script is
  ; monotonic and refuses to downgrade a newer Engine left by a later
  ; Desktop version.
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

  Call SeedOrMigrateAutostart

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

  Call StartEngineAndManager
SectionEnd

Section "Uninstall"
  SetShellVarContext current

  ; Stop processes before deleting files so nothing is locked.
  !insertmacro KillKosmosProcesses

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
  !insertmacro DeleteOldElectronRunValues

  ; Remove only the application payload we ship. User data in %APPDATA%\Kosmos
  ; and %LOCALAPPDATA%\Kosmos (including the Engine) is intentionally kept.
  RMDir /r "$INSTDIR\resources"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
