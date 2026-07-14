# Problems

## P1. Первый visual update превысил timeout screenshot capture

- Команда: `rtk bun run visual:eden -- --update-snapshots`
- Результат: `FAIL` на `toHaveScreenshot("eden-everything-mixed.png")` после 15 секунд.
- Наблюдение: до screenshot тест успешно прошёл cold reopen, дождался `everything-view`, четырёх карточек, двух декодированных data-URI изображений и загруженных шрифтов. Ошибка произошла внутри первого capture нового baseline, а не при загрузке данных или layout assertions.
- Первая попытка fix: дать cold Electron window 500 ms settle, увеличить timeout screenshot до 30 секунд и общий timeout visual case до 60 секунд.
- Reverify 1: `FAIL` — readiness guards по-прежнему прошли, но CDP `page.screenshot` не завершился и за 30 секунд. Значит причина не в медленной загрузке UI.
- Минимальный fix 2: использовать существующий repo-паттерн `BrowserWindow.capturePage()` для скрытого повторно созданного Electron window, а PNG Buffer сравнивать стандартным Playwright `toMatchSnapshot` с тем же pixel threshold.
- Reverify 2: `PASS` — update-run создал `eden-everything-mixed-win32.png` и завершился за 4.3 s; повторный run без update прошёл за 4.4 s. Baseline дополнительно открыт и проверен визуально.

## P2. Дополнительный stale TrailingParagraph e2e вне acceptance scope

- Дополнительная команда после обязательных gates: `rtk bunx playwright test tests/e2e/eden-trailing-paragraph.spec.ts`.
- Результат: helper успешно открыл записи через новое always-home поведение (`mounted=true`), 3 tests passed, 2 failed, 1 skipped.
- Оба FAIL проверяют инвариант удалённого `TrailingParagraph`: header самого spec ссылается на отсутствующий `products/eden/src/TrailingParagraph.ts`, а поиск по текущему репо не находит такой extension или source file. Ошибки не относятся к Everything, `book_obj`, ARK, navigation load или обязательным AC-командам.
- Минимальная in-scope адаптация: `openNoteViaReload()` больше не использует запрещённый last-entry restore; после reload он находит существующий Pinia store в production `file://` renderer, предзагружает полную запись и вызывает канонический `navigateTo()`. Лог подтверждает `mounted=true`.
- Решение: не восстанавливать удалённую editor feature и не переписывать её stale spec в этой задаче. Обязательные AC1–AC6 и независимый verifier остаются `PASS`.
