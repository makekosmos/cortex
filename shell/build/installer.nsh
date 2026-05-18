; Kepler NSIS customisation — restore hosts file на uninstall.
;
; electron-builder подхватывает этот .nsh через `nsis.include` config.
; См. https://www.electron.build/configuration/nsis для доступных hooks:
;   - customInstall   — после copy files
;   - customUnInstall — перед удалением files (helper.exe ещё доступен)
;   - customRemoveFiles — финальная очистка

!macro customUnInstall
  ; Если юзер активировал focus mode и в hosts остались kepler-managed
  ; entries — нужно очистить ДО deletion helper.exe. Запускаем helper с
  ; op=reset; UAC уже active т.к. uninstaller сам elevated (NSIS oneClick).
  ;
  ; Помещаем JSON request в %TEMP%\kepler-focus-uninstall-req.json,
  ; запускаем helper с --input. Игнорируем exit code (если hosts уже
  ; чистый — helper всё равно выйдет ok без изменений).

  ; ${INSTDIR}\resources\kepler-focus-helper.exe — путь как в production install.
  IfFileExists "$INSTDIR\resources\kepler-focus-helper.exe" 0 skip_hosts_reset

  ; Write request JSON.
  FileOpen $0 "$TEMP\kepler-focus-uninstall-req.json" w
  FileWrite $0 '{"op":"reset"}'
  FileClose $0

  ; Run helper synchronously. nsExec::ExecToLog captures output.
  nsExec::ExecToLog '"$INSTDIR\resources\kepler-focus-helper.exe" --input "$TEMP\kepler-focus-uninstall-req.json" --output "$TEMP\kepler-focus-uninstall-resp.json"'
  Pop $1

  ; Cleanup temp файлов.
  Delete "$TEMP\kepler-focus-uninstall-req.json"
  Delete "$TEMP\kepler-focus-uninstall-resp.json"

  skip_hosts_reset:
!macroend
