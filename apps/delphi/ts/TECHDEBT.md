# Delphi Desktop — Tech Debt / TODO

## Migration: React -> Vue 3 + Vapor

- [ ] Мигрировать с React 19 на Vue 3 (latest) с включённым Vapor mode
- [ ] Заменить Zustand на Pinia
- [ ] Заменить react-router-dom на vue-router
- [ ] Заменить Radix UI на Vue-совместимые headless компоненты (Reka UI и т.д.)
- [ ] Заменить lucide-react на lucide-vue-next
- [ ] Адаптировать @kosmos/ui под Vue
- [ ] Держать TS sync-утилиты (`hlc`, `lan-protocol`, `ark-types`) чистыми и фреймворк-агностичными

## Mobile: уход с React Native

- [ ] Определиться с целевым стеком: Flutter или Kotlin (Compose Multiplatform)
- [ ] Оценить переносимость текущей sync-логики на Dart/Kotlin
