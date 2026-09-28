; Kosmos NSIS customisation — migration cleanup + restore hosts file on uninstall.
;
; electron-builder подхватывает этот .nsh через `nsis.include` config.
; См. https://www.electron.build/configuration/nsis для доступных hooks:
;   - customInstall   — после copy files
;   - customUnInstall — перед удалением files (helper.exe ещё доступен)
;   - customRemoveFiles — финальная очистка

!macro customInstall
  ; KOS-233: Engine is built from this same tree/commit and bundled locally —
  ; nothing is downloaded here. install-engine.ps1 installs it into the
  ; shared %LOCALAPPDATA%\Kosmos\Engine root (never downgrading a newer,
  ; already-verified install) and takes over an existing standalone
  ; "Kosmos Engine" installation left by the old separate installer.
  ; Aborting here prevents a successful-looking GUI install without its engine.
  StrCpy $R0 "$WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe"
  IfFileExists "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe" 0 +2
    StrCpy $R0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  nsExec::ExecToStack '"$R0" -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\resources\install-engine.ps1" -Archive "$INSTDIR\resources\Kosmos Engine.zip" -Manifest "$INSTDIR\resources\engine-manifest.json" -TargetRoot "$LOCALAPPDATA\Kosmos\Engine"'
  Pop $0
  Pop $1
  StrCmp $0 "0" engine_ready
  Abort "Kosmos Engine installation failed: $1"
  engine_ready:
  ; Kosmos is the Shell application. Keep Manager as a separate Kosmos
  ; component and make Windows entry points launch the packaged Shell.
  Delete "$DESKTOP\CosCast.lnk"
  Delete "$SMPROGRAMS\CosCast.lnk"
  Delete "$DESKTOP\Kosmos.lnk"
  Delete "$SMPROGRAMS\Kosmos.lnk"
  CreateShortCut "$DESKTOP\Kosmos.lnk" "$INSTDIR\Kosmos.exe"
  CreateShortCut "$SMPROGRAMS\Kosmos.lnk" "$INSTDIR\Kosmos.exe"

  ; KOS-137: Agenda GPUI ships as a sibling component next to Manager.
  ; Guarded so a package built without components/agenda leaves no dead link.
  IfFileExists "$INSTDIR\resources\components\agenda\Kosmos Agenda.exe" 0 agenda_shortcut_done
    CreateShortCut "$SMPROGRAMS\Kosmos Agenda.lnk" "$INSTDIR\resources\components\agenda\Kosmos Agenda.exe"
  agenda_shortcut_done:

  ; KOS-156: Memoria GPUI ships as a sibling component next to Manager.
  ; Guarded so a package built without components/memoria leaves no dead link.
  IfFileExists "$INSTDIR\resources\components\memoria\Kosmos Memoria.exe" 0 memoria_shortcut_done
    CreateShortCut "$SMPROGRAMS\Kosmos Memoria.lnk" "$INSTDIR\resources\components\memoria\Kosmos Memoria.exe"
  memoria_shortcut_done:

  ; Kepler → Kosmos product rename. Legacy shortcuts are removed best-effort;
  ; user data remains untouched.
  Delete "$DESKTOP\Kepler.lnk"
  Delete "$SMPROGRAMS\Kepler.lnk"

  ; Remove legacy autostart entries. The GUI resolves the shared engine at boot;
  ; the engine remains independently installed and is never removed here.
  ${ifNot} ${isUpdated}
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.kazui.kepler"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kepler"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KeplerKosmos"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KosmosKepler"
  ${else}
    ; См. postmortems.md § 2026-08-01: не оставлять два владельца autostart.
    ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos"
    StrCmp $0 "" legacy_com_kazui_check legacy_autostart_present
    legacy_com_kazui_check:
      ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.kazui.kepler"
      StrCmp $0 "" legacy_kepler_check legacy_autostart_present
    legacy_kepler_check:
      ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kepler"
      StrCmp $0 "" legacy_keplerkosmos_check legacy_autostart_present
    legacy_keplerkosmos_check:
      ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KeplerKosmos"
      StrCmp $0 "" legacy_kosmoskepler_check legacy_autostart_present
    legacy_kosmoskepler_check:
      ReadRegStr $0 HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KosmosKepler"
      StrCmp $0 "" legacy_autostart_cleanup legacy_autostart_present
    legacy_autostart_present:
    legacy_autostart_cleanup:
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.kazui.kepler"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kepler"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KeplerKosmos"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KosmosKepler"
  ${endIf}
!macroend

!macro customUnInstall
  Delete "$DESKTOP\CosCast.lnk"
  Delete "$SMPROGRAMS\CosCast.lnk"
  Delete "$DESKTOP\Kosmos.lnk"
  Delete "$SMPROGRAMS\Kosmos.lnk"
  Delete "$SMPROGRAMS\Kosmos Agenda.lnk"
  Delete "$SMPROGRAMS\Kosmos Memoria.lnk"

  ; Если юзер активировал focus mode и в hosts остались managed
  ; entries — нужно очистить ДО deletion helper.exe. Запускаем helper с
  ; op=reset; UAC уже active т.к. uninstaller сам elevated (NSIS oneClick).
  ;
  ; Помещаем JSON request в %TEMP%\kosmos-helper-uninstall-req.json,
  ; запускаем helper с --input. Игнорируем exit code (если hosts уже
  ; чистый — helper всё равно выйдет ok без изменений).

  ; ${INSTDIR}\resources\Kosmos Helper.exe — путь как в production install.
  IfFileExists "$INSTDIR\resources\Kosmos Helper.exe" 0 try_legacy_helper

  ; Write request JSON.
  FileOpen $0 "$TEMP\kosmos-helper-uninstall-req.json" w
  FileWrite $0 '{"op":"reset"}'
  FileClose $0

  ; Run helper synchronously. nsExec::ExecToLog captures output.
  nsExec::ExecToLog '"$INSTDIR\resources\Kosmos Helper.exe" --input "$TEMP\kosmos-helper-uninstall-req.json" --output "$TEMP\kosmos-helper-uninstall-resp.json"'
  Pop $1

  ; Cleanup temp файлов.
  Delete "$TEMP\kosmos-helper-uninstall-req.json"
  Delete "$TEMP\kosmos-helper-uninstall-resp.json"
  Goto skip_hosts_reset

  try_legacy_helper:
  IfFileExists "$INSTDIR\resources\kepler-focus-helper.exe" 0 skip_hosts_reset
  FileOpen $0 "$TEMP\kosmos-helper-uninstall-req.json" w
  FileWrite $0 '{"op":"reset"}'
  FileClose $0
  nsExec::ExecToLog '"$INSTDIR\resources\kepler-focus-helper.exe" --input "$TEMP\kosmos-helper-uninstall-req.json" --output "$TEMP\kosmos-helper-uninstall-resp.json"'
  Pop $1
  Delete "$TEMP\kosmos-helper-uninstall-req.json"
  Delete "$TEMP\kosmos-helper-uninstall-resp.json"

  skip_hosts_reset:
!macroend
