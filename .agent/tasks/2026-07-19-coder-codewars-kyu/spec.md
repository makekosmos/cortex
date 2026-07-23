# Coder Codewars Kyu Distribution

## Goal

Упростить переключатель и gauge на странице «Кодер», а на вкладке Codewars показать количественное распределение завершённых kata по kyu.

## Scope

- Убрать pointer-cursor у переключателя LeetCode / Codewars.
- Убрать левую подпись и слово «Решено» из gauge LeetCode, увеличить центральное число.
- Дополнить существующую Codewars-синхронизацию рангом каждой kata из публичного Codewars API и один раз backfill-ить ранее сохранённые kata без поля ранга.
- Через существующий `CoderBreakdown` показать количество решённых kata по kyu.
- Не добавлять новые компоненты, зависимости, таблицы, RPC endpoints или direct SQL writes.

## Acceptance Criteria

**AC1.** Переключатель LeetCode / Codewars не задаёт `cursor: pointer` и сохраняет доступное button-поведение.

**AC2.** Gauge LeetCode не показывает левый текстовый блок и подпись «Решено»; центральное значение визуально крупнее прежнего `1rem`.

**AC3.** Codewars sync получает `rank` каждой импортируемой kata через официальный endpoint `/api/v1/code-challenges/{challenge}`, сохраняет поле в `coding_submission_obj` через существующий ARK `upsert_object` и при обнаружении старых Codewars-объектов без поля `rank` выполняет полный одноразовый backfill.

**AC4.** Вкладка Codewars показывает количественное распределение завершённых kata по kyu; строки упорядочены от `8 kyu` к `1 kyu`, kata без опубликованного ранга не создают ложную категорию.

**AC5.** Проходят targeted Rust/TypeScript tests, desktop typecheck, `ark:guard:writes` и визуальная проверка вкладок LeetCode и Codewars.
