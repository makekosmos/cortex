---
name: typecheck
description: Проверить актуальные TypeScript/Vue/ARK части Kosmos на ошибки компиляции. Use when user asks for typecheck or quick compile verification.
disable-model-invocation: true
allowed-tools: Bash
---

Проверь актуальные desktop/ARK части репозитория. Не используй legacy пути `apps/delphi/ts` или `apps/delphi/swift`.

## Быстрая desktop-проверка

```bash
rtk bun run desktop:typecheck
rtk bun run --cwd core/ark/packages/ark typecheck
```

## Если менялся Delphi desktop extension

```bash
rtk bun run ext:build delphi
```

## Если менялся Android stack

```bash
cd incubator/mobile/ark-service && rtk ./gradlew build
cd incubator/mobile/delphi && rtk ./gradlew build
```

В финале выведи краткую сводку: что прошло, что упало, первые релевантные ошибки.
