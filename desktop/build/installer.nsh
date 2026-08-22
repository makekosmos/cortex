; Kosmos NSIS customisation — migration cleanup + restore hosts file on uninstall.
;
; electron-builder подхватывает этот .nsh через `nsis.include` config.
; См. https://www.electron.build/configuration/nsis для доступных hooks:
;   - customInstall   — после copy files
;   - customUnInstall — перед удалением files (helper.exe ещё доступен)
;   - customRemoveFiles — финальная очистка

!macro customInstall
  ; Kepler → Kosmos product rename. electron-builder создаст новые shortcuts
  ; `Kosmos`; legacy shortcuts убираем best-effort, user data не трогаем.
  Delete "$DESKTOP\Kepler.lnk"
  Delete "$SMPROGRAMS\Kepler.lnk"

  ; Engine включён вместе с Windows по умолчанию только на новой установке.
  ; На update мигрируем только реально включённый legacy Run entry.
  ${ifNot} ${isUpdated}
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos Engine" '"$INSTDIR\resources\Kosmos Runtime.exe" --start'
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
      WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos Engine" '"$INSTDIR\resources\Kosmos Runtime.exe" --start'
    legacy_autostart_cleanup:
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.kazui.kepler"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kepler"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KeplerKosmos"
      DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KosmosKepler"
  ${endIf}
!macroend

!macro customUnInstall
  ; Runtime держится независимо от Electron, поэтому освобождаем executable
  ; перед update/uninstall.
  IfFileExists "$INSTDIR\resources\Kosmos Runtime.exe" 0 runtime_stopped
  nsExec::ExecToLog '"$INSTDIR\resources\Kosmos Runtime.exe" --shutdown'
  Pop $1
  runtime_stopped:

  ${ifNot} ${isUpdated}
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Kosmos Engine"
  ${endIf}
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
