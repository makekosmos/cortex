---
name: typecheck
description: Проверить TypeScript и Swift на ошибки компиляции
disable-model-invocation: true
allowed-tools: Bash
---

Проверь все приложения на ошибки компиляции:

1. **Delphi TS**:
   ```bash
   cd apps/delphi/ts && npx tsc --noEmit
   ```

2. **Delphi Swift** (если xcodebuild доступен):
   ```bash
   cd apps/delphi/swift && xcodebuild -scheme Delphi -destination 'platform=macOS' build 2>&1 | tail -20
   ```

3. Выведи сводку: что прошло, что упало, какие ошибки.
