# Headless Vite Port Cleanup

## Trigger

Когда background Vite/dev server запущен через Codex `exec` на Windows, а unified session не принимает `Ctrl+C`.

## Symptom

`write_stdin` возвращает `process interrupt is not supported`, и headless server продолжает слушать dev port.

## Do This

Останови только listener нужного известного порта в одном PowerShell-процессе:

```powershell
rtk powershell -NoProfile -Command '$processId=(Get-NetTCPConnection -LocalPort <PORT> -State Listen).OwningProcess; if ($processId) { Stop-Process -Id $processId }'
```

Затем проверь, что listener исчез. Команда должна использовать явно заданный dev port, а не поиск по имени процесса.

## Avoid

- Не делай broad `taskkill /IM node.exe` или `Stop-Process -Name node`: это остановит чужие dev sessions.
- Не запускай новый server на другом порту, оставляя старый процесс сиротой.
- Не открывай видимое окно PowerShell для headless visual verification.

## Promote To Skill When

Если тот же cleanup потребуется ещё в нескольких независимых Windows dev/visual workflows.
