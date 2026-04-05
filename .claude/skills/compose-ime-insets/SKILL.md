---
name: compose-ime-insets
description: Правильное позиционирование input bar над клавиатурой в Jetpack Compose с edge-to-edge. Используй когда пользователь жалуется на зазор между инпутом и клавиатурой, двойной отступ, или сдвиг UI при открытии клавиатуры.
---

# Compose IME Insets — input bar над клавиатурой

## Проблема

В edge-to-edge приложении с Scaffold + NavigationBar (таб-бар) + input bar внизу экрана — при открытии клавиатуры появляется зазор между input bar и клавиатурой. Причина: двойное потребление insets.

## Диагностика

Прежде чем фиксить — замерь. Добавь в Scaffold content:

```kotlin
val density = LocalDensity.current
val imeBottom = WindowInsets.ime.getBottom(density)
val navBottom = WindowInsets.navigationBars.getBottom(density)
Log.d("INSETS", "ime=$imeBottom nav=$navBottom paddingBottom=${paddingValues.calculateBottomPadding()}")
```

Открой/закрой клавиатуру, посмотри `adb logcat -s INSETS`. Типичные значения:
- `ime=825, nav=63, paddingBottom=80dp` — клавиатура открыта
- `ime=0, nav=63, paddingBottom=80dp` — клавиатура закрыта

Зазор = `paddingBottom` (высота таб-бара). Scaffold даёт bottom padding за таб-бар, потом imePadding() добавляет поверх.

## Решение (проверено на Android 14-15)

### Манифест

```xml
<activity android:windowSoftInputMode="adjustResize">
```

**adjustResize** обязателен — без него `WindowInsets.ime` не обновляется.

### Activity

```kotlin
override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)
    enableEdgeToEdge()
    // ...
}
```

### Структура Scaffold

```kotlin
// Внешний Scaffold (с таб-баром)
@OptIn(ExperimentalLayoutApi::class)
val isImeVisible = WindowInsets.isImeVisible

Scaffold(
    contentWindowInsets = WindowInsets(0),  // НЕ потребляет insets сам
    bottomBar = {
        // СКРЫТЬ таб-бар при клавиатуре — убирает двойной отступ
        if (showBottomBar && !isImeVisible) {
            NavigationBar(...) { ... }
            // NavigationBar сама потребляет navigationBars — не обнулять windowInsets
        }
    },
) { paddingValues ->
    // paddingValues.bottom = высота таб-бара (или 0 если скрыт)
    Content(Modifier.padding(paddingValues))
}
```

### Экран с input bar

```kotlin
Scaffold(
    contentWindowInsets = WindowInsets(0, 0, 0, 0),
    topBar = {
        TopAppBar(title = { Text("Title") })
        // TopAppBar потребляет statusBars по умолчанию — НЕ обнулять windowInsets
    },
) { paddingValues ->
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(paddingValues)
            .imePadding(),  // поднимает ВСЮ колонку при клавиатуре
    ) {
        LazyColumn(modifier = Modifier.weight(1f)) { ... }  // сжимается

        AnimatedVisibility(visible = showInput) {
            InputBar()  // БЕЗ imePadding, navigationBarsPadding, windowInsetsPadding
        }
    }
}
```

## Правила

1. **imePadding()** — ровно ОДНО место в цепочке, на общем контейнере (Column)
2. **Input bar** — никаких insets modifier. Он просто внизу Column.
3. **Таб-бар скрывается** при `WindowInsets.isImeVisible` — стандартный паттерн (Telegram, WhatsApp)
4. **TopAppBar** потребляет statusBars по умолчанию — не трогать
5. **NavigationBar** потребляет navigationBars по умолчанию — не трогать
6. **Scaffold contentWindowInsets = WindowInsets(0)** — Scaffold не потребляет insets сам
7. **Не вычитай insets вручную** — `ime.exclude(nav)`, `max(ime, nav) - nav` и т.д. ломаются на разных клавиатурах

## Частые ошибки

| Ошибка | Результат |
|--------|-----------|
| `imePadding()` на input bar + `adjustResize` | Двойной отступ |
| `navigationBarsPadding()` внутри input bar | +63px зазор |
| `windowInsetsPadding(ime.union(nav))` на Surface | Фон Surface тянется в зазор |
| Ручной `padding(bottom = ime - nav)` | Зазор = nav bar |
| `adjustNothing` | ime insets могут не репортиться |
| Таб-бар НЕ скрыт + imePadding | Зазор = высота таб-бара (~80dp) |

## ContentProvider + ContentObserver

Если данные приходят через ContentProvider (IPC):
- **ContentObserver.onChange()** вызывается на main thread
- Никогда не делай `contentResolver.query()` внутри onChange напрямую
- Используй `ioScope.launch { val result = query(); trySend(result) }` в callbackFlow
- Каждый Flow создаёт отдельный ContentObserver + IPC — не дублируй потоки
- UriMatcher: `*` для UUID, не `#` (# = только числа)
