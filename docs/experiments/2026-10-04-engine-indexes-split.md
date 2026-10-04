# Замеры после выделения engine-indexes (KOS-335, 2026-10-04)

Сравнение с baseline KOS-330 и методология как у KOS-334:
[`2026-10-04-engine-build-baseline.md`](./2026-10-04-engine-build-baseline.md),
[`2026-10-04-engine-dictation-split.md`](./2026-10-04-engine-dictation-split.md).
Та же машина, отдельный `CARGO_TARGET_DIR`, тёплый mbx, marker-fn в конец
файла, `git restore` после замера.

## Что изменилось

`runtime/src/{file_index,app_index,privileged}/` (~11k LOC) →
`runtime/crates/engine-indexes`. Фасад
`pub use engine_indexes::{app_index, file_index, privileged};` в
`runtime/src/lib.rs` — все `crate::<module>::*` пути в engine работают без
правок. Внутри крейта `pub(crate) use engine_base::{brand, priority};`
сохраняет прежние `crate::*` пути.

Цикл file_index↔privileged (`scanner/ntfs.rs` → pipe-протокол сервиса,
`ntfs_scan.rs` → `scanner::path_contains_noisy_folder`) оставлен внутри
одного крейта — это предпочтительный вариант из тикета: пара сканирует один
и тот же MFT и делится ~12 хелперами, разрыв back-edge создал бы дубли.

Два обратных ребра в engine разрешены переносом хелперов в крейт с
ре-экспортом на старых путях:

- `auth::process_image_path` → `engine_indexes::process_image`
  (ошибка `ProcessImageError`, engine-обёртка мапит на `AuthError`);
- `engine_versions::engine_root_of_exe` → `engine_indexes::install_layout`
  (engine `prune.rs` ре-экспортирует).

Index-only deps переехали с кодом: `notify`, `ignore`, `globset`, `walkdir`,
`ntfs-reader`, `windows-service`. `lnk`/`image`/`sha2`/`semver`/`rusqlite` —
в обоих манифестах (shared). Из engine-манифеста убраны только-индексные
windows-фичи (UWP `ApplicationModel`/`Management`, `Foundation*`/`Storage*`,
`Win32_Graphics_Gdi`, `Win32_NetworkManagement_WindowsFirewall`,
`Win32_System_Pipes`, `Win32_Media_*`, `Win32_Security_Authorization`,
`Win32_System_WindowsProgramming`) — по текстовому grep в оставшемся коде
engine они не используются.

NTFS-дедуп: `volume_path`/`user_path`/`strip_volume_prefix`/`mft_to_*` и
тест `converts_nt_device_paths_to_user_paths` были продублированы в
`file_index/scanner/ntfs.rs` и `privileged/ntfs_scan.rs` — теперь один
`crate::ntfs_common` (~90 строк вместо ~2×60).

## Результаты, секунды

| Случай                                          | До split | После split |
| ----------------------------------------------- | -------: | ----------: |
| incr-edit в file_index (стабильный прогон)      |      7.2 |   6.9–7.5 |
| test-build после правки в file_index            |     ~66 ¹|       3.6 |
| `cargo nextest run -p engine-indexes`           |      n/a |   100 тестов, 0.6 прогон |

¹ Baseline KOS-330/334: правка внутри engine-lib пересобирала весь
test-harness engine. Теперь правка в indexes собирает только harness
крейта — билд engine не нужен.

Изоляция подтверждена логом: правка в `file_index` пересобирает только
`engine-indexes` + релинк bin (engine lib не трогается). Incr-edit по
времени почти flat — на этой машине ~7 с упирается в линк bin; реальный
выигрыш — test-build (66 → 3.6 с) и то, что engine-lib больше не
пересобирается при правках в indexes.

## Решения для эпика KOS-329

1. Три модуля в одном крейте (не три крейта) — цикл file_index↔privileged
   и общий NTFS-код делают раздельные крейты искусственными.
2. Обратные рёбра в engine закрываются переносом маленьких pure-helper'ов
   (`process_image_path`, `engine_root_of_exe`) с ре-экспортом на старом
   пути — ни одного call-site в engine не изменено.
