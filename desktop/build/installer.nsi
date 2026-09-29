; Mundus Desktop installer (Engine + Manager component) — no Electron.
; Agenda, Memoria and Dictation are native apps installed by the Engine from
; their GitHub releases (KOS-265); only components\manager ships here.
;
; Usage:
;   makensis.exe /DVERSION=1.2.3 /DSTAGE_DIR=C:\...\installer-stage /DOUT_FILE=C:\...\Mundus-Setup-1.2.3.exe installer.nsi

!ifndef VERSION
  !error "VERSION must be defined via /DVERSION=<semver>"
!endif
!ifndef STAGE_DIR
  !error "STAGE_DIR must be defined via /DSTAGE_DIR=<path>"
!endif
!ifndef OUT_FILE
  !error "OUT_FILE must be defined via /DOUT_FILE=<path>"
!endif

; --- Brand block (mirrors desktop/scripts/brand.mjs / runtime/src/brand.rs) --
!define APP_NAME "Mundus"
!define PUBLISHER "Kazui"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Mundus"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define RUN_VALUE "Mundus Engine"
!define ENGINE_ARCHIVE "Mundus Engine.zip"
!define ENGINE_ROOT "$LOCALAPPDATA\Mundus\Engine"
!define APPS_ROOT "$LOCALAPPDATA\Mundus\Apps"
!define MANAGER_EXE "Mundus Manager.exe"
; Privileged Engine service (one-time UAC grant). The name mirrors
; brand::SYSTEM_SERVICE_NAME in the Engine — keep in sync.
!define PRIVILEGED_SVC_NAME "MundusSystemSvc"
!define PRIVILEGED_SVC_EXE "$PROGRAMFILES64\Mundus\Service\mundus-privileged-service.exe"

Name "${APP_NAME} ${VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\Mundus"
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

; Current process names plus the ones a 0.9.x/Electron-era install may have
; left running. MIGRATION(KOS-267): remove the legacy names after 2026-11-01.
!macro KillProductProcesses
  nsExec::ExecToLog 'taskkill /F /IM Mundus.exe'
  nsExec::ExecToLog 'taskkill /F /IM mundus-engine.exe'
  nsExec::ExecToLog 'taskkill /F /IM "Mundus Manager.exe"'
  ; Store-installed native apps (KOS-265) — must not hold their dir while the
  ; Engine or an uninstall replaces/removes it.
  nsExec::ExecToLog 'taskkill /F /IM agenda-gpui.exe'
  nsExec::ExecToLog 'taskkill /F /IM memoria-gpui.exe'
  nsExec::ExecToLog 'taskkill /F /IM dictation-gpui.exe'
  nsExec::ExecToLog 'taskkill /F /IM "Agenda.exe"'                    ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM "Memoria.exe"'                   ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM "Dictation.exe"'                 ; MIGRATION(KOS-267)
  ; 0.9.x installs leave an orphaned ark-core-rpc.exe child holding the DB.
  nsExec::ExecToLog 'taskkill /F /IM ark-core-rpc.exe'            ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM Kosmos.exe'                  ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM kepler-backend.exe'          ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Manager.exe"'        ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Agenda.exe"'         ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Memoria.exe"'        ; MIGRATION(KOS-267)
  nsExec::ExecToLog 'taskkill /F /IM "Kosmos Dictation.exe"'      ; MIGRATION(KOS-267)
  Sleep 500
!macroend

; Legacy HKCU Run values from every previous product generation. Read them
; before deleting — they carry the user's autostart preference.
; MIGRATION(KOS-267): remove after 2026-11-01.
!macro ReadOldAutostartDetected
  ReadRegStr $R0 HKCU "${RUN_KEY}" "com.kazui.kosmos"             ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "electron.app.Kosmos"          ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "Kosmos"                       ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "Kosmos Engine"                ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "com.kazui.kepler"             ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "Kepler"                       ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "KeplerKosmos"                 ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
  ReadRegStr $R0 HKCU "${RUN_KEY}" "KosmosKepler"                 ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R2 1
!macroend

; MIGRATION(KOS-267): remove after 2026-11-01.
!macro DeleteOldRunValues
  DeleteRegValue HKCU "${RUN_KEY}" "com.kazui.kosmos"             ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "electron.app.Kosmos"          ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "Kosmos"                       ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "Kosmos Engine"                ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "com.kazui.kepler"             ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "Kepler"                       ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "KeplerKosmos"                 ; MIGRATION(KOS-267)
  DeleteRegValue HKCU "${RUN_KEY}" "KosmosKepler"                 ; MIGRATION(KOS-267)
!macroend

Function .onInit
  ; Legacy electron-updater passes arguments like --updated and --force-run.
  ; NSIS does not recognise them, so we simply ignore them rather than failing.
FunctionEnd

; Returns:
;   $R2 = 1 if an old autostart Run value was present.
;   $R3 = 1 only if no previous install of any generation was detected.
Function DetectPreviousInstall
  StrCpy $R2 0
  StrCpy $R3 1

  ; Old Electron installation. MIGRATION(KOS-267): remove after 2026-11-01.
  IfFileExists "$LOCALAPPDATA\Programs\kepler-shell" 0 +2          ; MIGRATION(KOS-267)
    StrCpy $R3 0
  IfFileExists "$LOCALAPPDATA\Programs\Kosmos" 0 +2                ; MIGRATION(KOS-267)
    StrCpy $R3 0
  ; The Electron uninstall keys are the bare GUIDs — no braces.
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\4fe2b964-4d0e-5a72-8728-cca14468c9f0" "DisplayName" ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R3 0
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d" "DisplayName" ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R3 0
  ; MIGRATION(KOS-267): Kosmos-era NSIS registration and standalone Engine entry.
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Kosmos" "DisplayName" ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R3 0
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" "DisplayName" ; MIGRATION(KOS-267)
  StrCmp $R0 "" +2
    StrCpy $R3 0
  IfFileExists "$LOCALAPPDATA\Kosmos\Engine\current.json" 0 +2     ; MIGRATION(KOS-267)
    StrCpy $R3 0

  ; Current Mundus installation.
  ReadRegStr $R0 HKCU "${UNINSTALL_KEY}" "DisplayName"
  StrCmp $R0 "" +2
    StrCpy $R3 0
  IfFileExists "${ENGINE_ROOT}\current.json" 0 +2
    StrCpy $R3 0

  ; Any surviving old autostart value counts as a previous install and, if
  ; present, means we should migrate the autostart preference.
  !insertmacro ReadOldAutostartDetected
  IntCmp $R2 1 0 +2
    StrCpy $R3 0
FunctionEnd

; Seeds Engine autostart only on a fresh install, or migrates the old
; autostart preference when upgrading from a previous generation. Uses the
; Engine version actually installed by install-engine.ps1, never the Desktop
; VERSION.
Function SeedOrMigrateAutostart
  ; Only act for fresh installs or upgrades with autostart enabled.
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
  Exec '"$INSTDIR\resources\components\manager\${MANAGER_EXE}"'
manager_done:
FunctionEnd


; MIGRATION(KOS-267): remove after 2026-11-01. Before any old payload is
; deleted, record which bundled components (agenda/memoria/dictation) the
; previous generation shipped — the Engine reads this marker on first start
; to auto-install them as store apps. Written only when a component was
; found: re-running the installer before the Engine's first start must not
; overwrite a recorded "true" with an all-false marker. An absent marker and
; "nothing recorded" mean the same. A write failure only skips the
; auto-install — the wipe still proceeds.
Function RecordLegacyComponents
  ; $R4/$R5/$R6 = "true"/"false" per component; presence in EITHER the 0.9.x
  ; Programs\Kosmos layout or this installer's previous bundled layout counts.
  StrCpy $R4 "false"
  StrCpy $R5 "false"
  StrCpy $R6 "false"
  IfFileExists "$LOCALAPPDATA\Programs\Kosmos\resources\components\agenda\*.*" 0 +2   ; MIGRATION(KOS-267)
    StrCpy $R4 "true"
  IfFileExists "$INSTDIR\resources\components\agenda\*.*" 0 +2                          ; MIGRATION(KOS-267)
    StrCpy $R4 "true"
  IfFileExists "$LOCALAPPDATA\Programs\Kosmos\resources\components\memoria\*.*" 0 +2  ; MIGRATION(KOS-267)
    StrCpy $R5 "true"
  IfFileExists "$INSTDIR\resources\components\memoria\*.*" 0 +2                         ; MIGRATION(KOS-267)
    StrCpy $R5 "true"
  IfFileExists "$LOCALAPPDATA\Programs\Kosmos\resources\components\dictation\*.*" 0 +2 ; MIGRATION(KOS-267)
    StrCpy $R6 "true"
  IfFileExists "$INSTDIR\resources\components\dictation\*.*" 0 +2                       ; MIGRATION(KOS-267)
    StrCpy $R6 "true"
  StrCmp "$R4$R5$R6" "falsefalsefalse" record_done
  CreateDirectory "$LOCALAPPDATA\Mundus"
  ClearErrors
  FileOpen $R7 "$LOCALAPPDATA\Mundus\legacy-components.json" w
  IfErrors record_failed
  FileWrite $R7 '{"schema_version":1,"components":{"agenda":$R4,"memoria":$R5,"dictation":$R6}}'
  FileWrite $R7 '$\r$\n'
  FileClose $R7
  Goto record_done
record_failed:
  DetailPrint "legacy-components marker write failed — bundled-app migration skipped"
record_done:
FunctionEnd

Section "Install"
  SetShellVarContext current

  ; Determine whether this is a fresh install or a migration/upgrade before we
  ; remove any state.
  Call DetectPreviousInstall

  ; MIGRATION(KOS-267): must run before the Programs\Kosmos and
  ; $INSTDIR\resources wipes below — it records which bundled
  ; components existed so the Engine's first-start migration reads a marker,
  ; not the deleted directories.
  Call RecordLegacyComponents                                                       ; MIGRATION(KOS-267)

  StrCpy $R0 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"

  ; Stop all product processes (current and legacy names) before touching
  ; files or registry.
  !insertmacro KillProductProcesses

  ; MIGRATION(KOS-267): remove after 2026-11-01. Old autostart values are
  ; stale regardless of whether the legacy payload survived; remove them
  ; unconditionally now that DetectPreviousInstall has read them for the
  ; migration decision.
  !insertmacro DeleteOldRunValues

  ; MIGRATION(KOS-267): upgrade from Electron or Kosmos-era installs: remove the old program
  ; files, its stale Apps & Features entries, and its shortcuts. We never run
  ; the old uninstaller and never touch %APPDATA%\Kosmos or
  ; %LOCALAPPDATA%\Kosmos — the Engine moves user data on first start.
  IfFileExists "$LOCALAPPDATA\Programs\kepler-shell" 0 +2          ; MIGRATION(KOS-267)
    RMDir /r "$LOCALAPPDATA\Programs\kepler-shell"                 ; MIGRATION(KOS-267)
  IfFileExists "$LOCALAPPDATA\Programs\Kosmos" 0 +2                ; MIGRATION(KOS-267)
    RMDir /r "$LOCALAPPDATA\Programs\Kosmos"                       ; MIGRATION(KOS-267)
  ; Bare GUID keys — the real Electron entries have no braces.
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\4fe2b964-4d0e-5a72-8728-cca14468c9f0" ; MIGRATION(KOS-267)
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\af85bd72-f4c8-5af3-a0fe-9aa1f0fa5a8d" ; MIGRATION(KOS-267)
  ; MIGRATION(KOS-267): Kosmos-era registration + the standalone Engine entry.
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Kosmos" ; MIGRATION(KOS-267)
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\KosmosEngine" ; MIGRATION(KOS-267)

  ; Bundled-era component shortcuts — the apps are store-installed now,
  ; so these point at files the upgrade deletes.
  Delete "$SMPROGRAMS\Agenda.lnk"
  Delete "$SMPROGRAMS\Memoria.lnk"
  Delete "$SMPROGRAMS\Dictation.lnk"

  ; Old shortcuts on Desktop and in the Start Menu.
  Delete "$DESKTOP\Kosmos.lnk"                                     ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos.lnk"                                  ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Agenda.lnk"                           ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Memoria.lnk"                          ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Dictation.lnk"                        ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Engine.lnk"                           ; MIGRATION(KOS-267)
  Delete "$DESKTOP\Kepler.lnk"                                     ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kepler.lnk"                                  ; MIGRATION(KOS-267)
  Delete "$DESKTOP\CosCast.lnk"                                    ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\CosCast.lnk"                                 ; MIGRATION(KOS-267)

  ; Replace the shipped application payload only. User data lives in
  ; %APPDATA%\Mundus and %LOCALAPPDATA%\Mundus and is never touched here.
  IfFileExists "$INSTDIR\resources\*.*" 0 +2
    RMDir /r "$INSTDIR\resources"
  Delete "$INSTDIR\Uninstall.exe"
  SetOutPath "$INSTDIR"
  CreateDirectory "$INSTDIR"
  File /r "${STAGE_DIR}\*"

  ; KOS-233: install the Engine bundled with this build. The script is
  ; monotonic and refuses to downgrade a newer Engine left by a later
  ; Desktop version.
  nsExec::ExecToStack '"$R0" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\resources\install-engine.ps1" -Archive "$INSTDIR\resources\${ENGINE_ARCHIVE}" -Manifest "$INSTDIR\resources\engine-manifest.json" -TargetRoot "${ENGINE_ROOT}"'
  Pop $0
  Pop $1
  StrCmp $0 "0" engine_ready
    Abort "Mundus Engine installation failed: $1"
  engine_ready:

  ; The old Engine payload under %LOCALAPPDATA%\Kosmos\Engine is only removed
  ; once the new Engine is installed and verified — never before, so a failed
  ; install leaves a working 0.9.x Engine.
  IfFileExists "$LOCALAPPDATA\Kosmos\Engine\*.*" 0 +2              ; MIGRATION(KOS-267)
    RMDir /r "$LOCALAPPDATA\Kosmos\Engine"                         ; MIGRATION(KOS-267)

  ; Windows entry points open the GPUI Manager.
  CreateShortCut "$SMPROGRAMS\Mundus.lnk" "$INSTDIR\resources\components\manager\${MANAGER_EXE}" "" "$INSTDIR\resources\icon.ico" 0
  CreateShortCut "$DESKTOP\Mundus.lnk" "$INSTDIR\resources\components\manager\${MANAGER_EXE}" "" "$INSTDIR\resources\icon.ico" 0

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

; If the privileged service is registered, offer one elevated uninstall via
; the stable service copy (the Engine tree under %LOCALAPPDATA% may already
; be gone). Declining the UAC prompt leaves the service installed but inert
; — the pipe accepts nobody until re-enabled.
Function un.RemovePrivilegedService
  ; `sc query` runs unelevated — skip the UAC round-trip entirely when no
  ; service is registered.
  nsExec::ExecToStack 'sc.exe query "${PRIVILEGED_SVC_NAME}"'
  Pop $R0
  Pop $R1
  StrCmp $R0 "0" 0 done
    IfFileExists "${PRIVILEGED_SVC_EXE}" 0 done
      ExecShell "runas" "${PRIVILEGED_SVC_EXE}" "privileged uninstall"
      Pop $R0
      ; ExecShell reports "error" when the user declines the UAC prompt or the
      ; launch fails — either way the service stays; say so and move on.
      StrCmp $R0 "error" 0 done
        DetailPrint "Privileged service left installed (elevation declined)"
        IfSilent +2
          MessageBox MB_ICONEXCLAMATION|MB_OK "The Mundus privileged service is still installed. To remove it later, run as administrator: $\r$\n${PRIVILEGED_SVC_EXE} privileged uninstall"
  done:
FunctionEnd

Section "Uninstall"
  SetShellVarContext current

  ; Stop processes (current and legacy names) before deleting files so
  ; nothing is locked.
  !insertmacro KillProductProcesses

  ; One UAC prompt to remove the service before files are cleaned.
  Call un.RemovePrivilegedService

  Delete "$DESKTOP\Mundus.lnk"
  Delete "$SMPROGRAMS\Mundus.lnk"
  Delete "$SMPROGRAMS\Agenda.lnk"
  Delete "$SMPROGRAMS\Memoria.lnk"
  Delete "$SMPROGRAMS\Dictation.lnk"
  ; Legacy shortcut names. MIGRATION(KOS-267): remove after 2026-11-01.
  Delete "$DESKTOP\Kosmos.lnk"                                     ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos.lnk"                                  ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Agenda.lnk"                           ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Memoria.lnk"                          ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Dictation.lnk"                        ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kosmos Engine.lnk"                           ; MIGRATION(KOS-267)
  Delete "$DESKTOP\Kepler.lnk"                                     ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\Kepler.lnk"                                  ; MIGRATION(KOS-267)
  Delete "$DESKTOP\CosCast.lnk"                                    ; MIGRATION(KOS-267)
  Delete "$SMPROGRAMS\CosCast.lnk"                                 ; MIGRATION(KOS-267)

  DeleteRegValue HKCU "${RUN_KEY}" "${RUN_VALUE}"
  !insertmacro DeleteOldRunValues

  ; Remove the payload we ship and the store-installed native apps (program
  ; files, not user data). The rest of %APPDATA%\Mundus and
  ; %LOCALAPPDATA%\Mundus (including the Engine) is intentionally kept.
  RMDir /r "${APPS_ROOT}"
  RMDir /r "$INSTDIR\resources"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
