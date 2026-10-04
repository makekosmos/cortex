# Общий кэш сборки: свежие worktree не должны быть холодными (KOS-341, 2026-10-04)

Цель: понять, почему mbx на части конфигураций пропускал кэш целиком
(599/605 «bypassed» в замере 2026-09-30), и доказать, что второй свежий
worktree с прогретым кэшем получает >90% попаданий и собирается ≥3× быстрее
чистой сборки.

## Диагноз

Замер 2026-09-30 показал: на Windows любой линковщик, заданный через env
(`CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER`, `RUSTFLAGS`), mbx не может
смоделировать в ключе кэша (`rustc codegen option is not modeled by the cache
adapter: linker`) и пропускает кэш целиком. На Linux та же ловушка была бы у
`RUSTFLAGS` / `CARGO_ENCODED_RUSTFLAGS` / `CARGO_TARGET_*_LINKER` из
окружения: значение env попадает в ключ (или делает юнит немоделируемым), и
любая разница в окружении между worktree/агентами обнуляет попадания.

KOS-328 уже перенёс lld в `.cargo/config.toml`
(`-C link-arg=-fuse-ld=lld` для `x86_64-unknown-linux-gnu`). Файл конфига
одинаков в каждом worktree, флаг входит в смоделированную часть ключа — кэш
работает. Проверено `grep` по репозиторию: ни скрипты, ни package.json, ни
доки не выставляют `RUSTFLAGS`/linker-переменные через env. В `.cargo/config.toml`
добавлен комментарий, запрещающий env-флаги (единственное изменение кода в
этом слайсе).

## Условия

- Машина: Linux VM (Debian 13, kernel 6.12.94+), Intel Xeon, 8 vCPU, 15 ГБ —
  та же, что в baseline KOS-330.
- Toolchain: stable 1.95.0, lld через `.cargo/config.toml`.
- mbx 1.15.0, кэш `/home/box/.cache/mbx` прогрет сборками worktree
  KOS-330/332/344 за тот же день.
- Код: `origin/main` @ `b53496eb` (после KOS-330).
- Холодная сборка: свежий worktree, прямой cargo (rustup-proxy, минуя
  shim mbx), `cargo fetch` заранее, отдельный `target/`.
- Тёплая сборка: второй свежий worktree того же коммита, cargo через shim mbx.
- Команда везде: `cargo build -p engine` (debug-профиль), секундомер вокруг
  вызова.

## Результаты

| Случай                                                     | Время   | hits | misses | bypassed |
| ---------------------------------------------------------- | ------: | ---: | -----: | -------: |
| cold, свежий worktree, cargo без mbx                       | 138.8 с |    — |      — |        — |
| kos-341 worktree, первый build этого коммита на машине, mbx |  66.2 с |  535 |      3 |        4 |
| второй свежий worktree того же коммита, mbx                |  13.0 с |  537 |      1 |        4 |

Второй свежий worktree: **537/542 юнитов из кэша (99.1%)**, **10.7× быстрее**
холодной сборки (138.8 → 13.0 с). Цели «>90% hits» и «≥3×» перевыполнены.

Даже первый build нового коммита (кос-341, 66 с) получил 98.7% попаданий:
промахнулись только крейты с genuinely изменившимися входами — `engine` и
`mundus_engine` (другой коммит/фингерпринты зависимостей) плюс `mime_guess`.

## Что осталось вне кэша (`mbx explain --last`)

1. **`mime_guess`** — единственный систематический промах. Его `build.rs`
   экспортирует `MIME_TYPES_GENERATED_PATH` как абсолютный путь в `OUT_DIR`
   текущего worktree, mbx включает значение env в ключ. Путь по определению
   различается между worktree — на уровне репозитория не лечится. Цена:
   один маленький крейт (~1 с).
2. **Крейты workspace при смене коммита** (`engine`, `mundus_engine`, …) —
   промахи законные: исходники и фингерпринты зависимостей реально другие.
   При том же коммите mbx нормализует путь worktree, и они попадают
   (в wt-warm hit).
3. **4 bypassed** — не компиляции, а служебные вызовы: cc compiler-query,
   cc missing-output, stdin, unsupported-crate-type. Работы не несут.

## Выводы

1. Холодные свежие worktree были следствием env-флагов, а не устройства mbx:
   env-значения либо немоделируемы (линковщик на Windows), либо входят в
   ключ и расходятся между окружениями. Правило: **все rustflags/linker
   настройки — только в `.cargo/config.toml`, никогда в env**.
2. На Linux после KOS-328 это правило уже выполнено — чинить было нечего,
   зафиксирован инвариант комментарием в конфиге и этим замером.
3. sccache (`RUSTC_WRAPPER`) не потребовался: mbx покрывает сценарий
   «свежий worktree» с ~99% попаданий, второй кэш-слой дал бы только
   дублирование ключей и бюджетов.
4. Остаточные промахи — ~1 крейт на worktree (`mime_guess`) плюс честная
   перекомпиляция изменённого кода. Дальше сокращать нечем без патчинга
   зависимости — не окупается.

## Как повторить

```bash
WT=/home/box/devin-runs/kos-341
git -C /workspace/wt/cortex-kos-341 worktree add $WT/wt-cold --detach b53496eb
git -C /workspace/wt/cortex-kos-341 worktree add $WT/wt-warm --detach b53496eb

# cold: прямой cargo, минуя shim mbx (PATH без ~/.local/share/mbx/bin)
cd $WT/wt-cold && /home/box/.cargo/bin/cargo fetch
time /home/box/.cargo/bin/cargo build -p engine

# warm: cargo через shim mbx
cd $WT/wt-warm && /home/box/.cargo/bin/cargo fetch
time cargo build -p engine
mbx explain --last
```
