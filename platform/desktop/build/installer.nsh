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
!macroend

!macro customUnInstall
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
