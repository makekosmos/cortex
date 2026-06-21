# Dictation pill + local STT fixes

Дата: 2026-06-20

Классификация: FULL_LOOP

## Scope

- `platform/desktop/electron/dictation-pill.ts`
- `platform/desktop/src/views/DictationPillView.vue`
- `platform/runtime/src/dictation/local.rs`
- Локальные тесты/preview только для этого surface.

## Acceptance Criteria

1. Dictation pill рендерится как прозрачный overlay без белой/непрозрачной прямоугольной подложки вокруг pill.
2. В состоянии записи кнопки на pill работают мышью:
   - cancel сбрасывает запись, вызывает backend cancel и закрывает pill;
   - submit останавливает запись и отправляет текущий WAV через `dictation.submit_audio`.
3. Pill не крадет фокус у активного окна перед inject path.
4. Локальный STT использует persistent `whisper-server.exe`, когда он доступен рядом с выбранным `whisper-cli.exe`, и не падает в cold-start CLI путь без явной причины.
5. Если `sample/handy` отсутствует в текущем workspace, это явно зафиксировано в финальном отчете; сравнение делается по имеющемуся fast-path server mode в repo.

## Checks

- TypeScript/build check для desktop surface или ближайший доступный scoped check.
- Rust unit tests для `platform/runtime/src/dictation/local.rs`.
- Visual verification preview для `#dictation-pill?preview=1`, включая прозрачность/размеры pill states.
