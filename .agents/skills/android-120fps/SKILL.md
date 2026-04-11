---
name: android-120fps
description: Диагностика и оптимизация производительности прокрутки в Android-приложениях на Kotlin + Jetpack Compose. Используй когда пользователь жалуется на лаги при скролле, dropped frames, jank, или хочет достичь 120fps на 120Hz дисплеях.
---

# Kotlin Android Compose — оптимизация 120fps

## Диагностика — с чего начинать

```bash
# Статистика dropped frames по пакету (первый шаг)
adb shell dumpsys gfxinfo <package.name>

# Собрать framestats CSV для анализа фаз
adb shell dumpsys gfxinfo <package.name> framestats

# Perfetto trace (подробнее — см. раздел ниже)
```

Developer Options на телефоне:
- **Profile GPU Rendering** → On screen as bars (красные столбики = jank)

Ключевые метрики в `gfxinfo`:
- `Janky frames` — % дропнутых кадров (цель < 5%)
- `90th percentile` — 90-й перцентиль времени кадра (цель < 8.3ms при 120Hz)
- В framestats смотри колонку `anim_phase` (= Compose рекомпозиция + layout)

---

## Правило №0: Debug vs Release

**Debug build в 2–5× медленнее release** из-за интерпретатора и отсутствия R8/AOT.
Всегда тестируй производительность на `releaseDebug` build type:

```kotlin
// app/build.gradle.kts
buildTypes {
    release {
        isMinifyEnabled = true
        isShrinkResources = true
        proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
    }
    create("releaseDebug") {
        initWith(getByName("release"))
        isDebuggable = true
        signingConfig = signingConfigs.getByName("debug")
        matchingFallbacks += listOf("release")
    }
}
```

```bash
./gradlew assembleReleaseDebug
adb install app/build/outputs/apk/releaseDebug/app-releaseDebug.apk
```

---

## Jetpack Compose

### 1. Стабильность — главное

Compose пропускает рекомпозицию только если все параметры **стабильны**.

```kotlin
// Помечай data class как @Immutable если все поля val и неизменяемые
@Immutable
data class TodoItem(val id: String, val title: String, ...)

// List<T> нестабилен — помечай содержащий класс @Immutable
@Immutable
data class RecurrenceData(val daysOfWeek: List<Int>? = null, ...)
```

**Kotlin 2.0+ (новый bundled Compose compiler): strong skipping включён по умолчанию.**
Лямбды автоматически оборачиваются в `remember`, нестабильные параметры сравниваются по ссылке.
Явное `remember(todo) { { viewModel.doAction(todo) } }` в `items {}` не нужно.

### 2. Проверка стабильности через Compose compiler reports

```kotlin
// app/build.gradle.kts
kotlinOptions {
    jvmTarget = "21"
    freeCompilerArgs += listOf(
        "-P", "plugin:androidx.compose.compiler.plugins.kotlin:reportsDestination=${project.layout.buildDirectory.get()}/compose_reports",
        "-P", "plugin:androidx.compose.compiler.plugins.kotlin:metricsDestination=${project.layout.buildDirectory.get()}/compose_reports",
    )
}
```

```bash
./gradlew :app:compileReleaseDebugKotlin
cat app/build/compose_reports/app_releaseDebug-composables.txt | grep -A5 "fun TodoRow"
```

Ищи:
- `restartable skippable` — ✅ хорошо, Compose может пропустить
- `unstable` у параметра — ⚠️ добавь `@Immutable` или `@Stable`

После анализа — убери флаги из `build.gradle.kts`.

### 3. LazyColumn: key + contentType + prefetch

```kotlin
@OptIn(ExperimentalFoundationApi::class)
val listState = rememberLazyListState(
    prefetchStrategy = LazyListPrefetchStrategy(8),  // pre-compose 8 items ahead
)

LazyColumn(
    state = listState,
    contentPadding = PaddingValues(vertical = 4.dp),
) {
    items(
        items = todos,
        key = { it.id },           // стабильные ключи → правильные анимации + skip
        contentType = { "todo" },  // один тип → Compose переиспользует слоты
    ) { todo ->
        TodoRow(todo = todo)
    }
}
```

⚠️ `beyondBoundsItemCount` — параметр не существует в Compose BOM 2025+. Используй `LazyListPrefetchStrategy`.

### 4. remember для вычислений внутри items

```kotlin
// Плохо — вызывается на каждую рекомпозицию
val formatted = formatDate(todo.scheduledDate)

// Хорошо — кешируется, пересчитывается только при изменении ключа
val formatted = remember(todo.scheduledDate) { formatDate(todo.scheduledDate) }
```

### 5. derivedStateOf для scroll-зависимого состояния

```kotlin
val listState = rememberLazyListState()

// Плохо — рекомпозиция на каждый пиксель скролла
val showFab = listState.firstVisibleItemIndex > 0

// Хорошо — рекомпозиция только при смене true/false
val showFab by remember { derivedStateOf { listState.firstVisibleItemIndex > 0 } }
```

### 6. Избегай аллокаций в compose scope

```kotlin
// Плохо — новый объект каждый кадр
items(list) {
    val color = MaterialTheme.colorScheme.primary.copy(alpha = 0.2f)  // аллокация!
}

// Хорошо — снаружи items или в remember
val dimColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.2f)  // вне items {}

// graphicsLayer лямбда — OK, не создаёт новый Modifier
Box(modifier = Modifier.graphicsLayer { alpha = value })
```

### 7. Surface vs Box

`Surface` читает `CompositionLocal` (elevation, colors) — лишние подписки. Для простых контейнеров используй `Box + Modifier.background`:

```kotlin
// Вместо Surface(color = priorityColor) { ... }
Box(modifier = Modifier.background(priorityColor, CircleShape)) { ... }
```

---

## Flow и Room

### distinctUntilChanged должен быть ДО flowOn

```kotlin
// Плохо — distinctUntilChanged запускается на Main thread
todoDao.getAll()
    .map { TodoFilterService.filter(smartList, it) }
    .flowOn(Dispatchers.Default)
    .distinctUntilChanged()  // ← на Main thread! бесполезно
    .stateIn(...)

// Хорошо — оба оператора на Default thread
todoDao.getAll()
    .map { TodoFilterService.filter(smartList, it) }
    .distinctUntilChanged()  // ← на Default thread (внутри flowOn scope)
    .flowOn(Dispatchers.Default)
    .stateIn(...)
```

### SQL-запросы вместо in-memory фильтрации

Для специфичных списков (Logbook, Trash) — SQL-запросы вместо `getAll()` + фильтр в памяти.
Room-инвалидация тригерится на уровне таблицы: `getAll()` пересчитывается при любой записи в `todos`.

```kotlin
// DAO
@Query("SELECT * FROM todos WHERE (isCompleted = 1 OR isCancelled = 1) AND isTrashed = 0 ORDER BY COALESCE(completedAt, cancelledAt) DESC")
fun getLogbook(): Flow<List<TodoItem>>

@Query("SELECT * FROM todos WHERE isTrashed = 1 ORDER BY createdAt DESC")
fun getTrash(): Flow<List<TodoItem>>

// ViewModel
val todos = when (smartList) {
    SmartList.LOGBOOK -> todoDao.getLogbook()
        .distinctUntilChanged()
        .flowOn(Dispatchers.Default)
    SmartList.TRASH -> todoDao.getTrash()
        .distinctUntilChanged()
        .flowOn(Dispatchers.Default)
    else -> todoDao.getAll()
        .map { TodoFilterService.filter(smartList, it) }
        .distinctUntilChanged()
        .flowOn(Dispatchers.Default)
}.stateIn(viewModelScope, SharingStarted.WhileSubscribed(5000), emptyList())
```

---

## Системный уровень

### Запрос 120Hz у системы (API 30+)

```kotlin
// MainActivity.onCreate
private fun requestHighRefreshRate() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
        val display = display ?: return
        val highestMode = display.supportedModes.maxByOrNull { it.refreshRate } ?: return
        window.attributes = window.attributes.apply {
            preferredDisplayModeId = highestMode.modeId
        }
    }
}
```

### Predictive Back Gesture (Android 13+)

```xml
<!-- AndroidManifest.xml -->
<application android:enableOnBackInvokedCallback="true" ...>
```

### Фиксация ориентации

```xml
<activity android:screenOrientation="portrait" ...>
```

### Renderer

На Snapdragon 778G+ Vulkan (`skiavk`) показал **деградацию** (90p: 16ms → 44ms).
Оставлять OpenGL по умолчанию, не включать Vulkan.

---

## Perfetto trace

Когда `gfxinfo` недостаточно — Perfetto для поиска конкретной фазы дропа.

```bash
# Конфиг записать на устройство
cat > /tmp/perf.txt << 'EOF'
buffers: { size_kb: 131072 }
data_sources: { config { name: "android.surfaceflinger.frametimeline" } }
data_sources: {
  config {
    name: "linux.ftrace"
    ftrace_config {
      ftrace_events: "sched/sched_switch"
      ftrace_events: "sched/sched_wakeup"
      atrace_categories: "gfx"
      atrace_categories: "view"
      atrace_apps: "com.your.package"
    }
  }
}
duration_ms: 20000
EOF
adb push /tmp/perf.txt /sdcard/perf.txt

# Запустить трейс (--background-wait запускает демон с правами на запись)
adb shell 'cat /sdcard/perf.txt | perfetto --background-wait -c - --txt -o /data/misc/perfetto-traces/trace.pftrace'

# ... скроллить ...

# Забрать файл
adb pull /data/misc/perfetto-traces/trace.pftrace /tmp/trace.pftrace
```

Открыть в **ui.perfetto.dev** → найти процесс приложения → смотреть `UI Thread` и `RenderThread`.

⚠️ `-o -` (stdout) работает но файл может быть неполным при обрыве соединения. Лучше `--background-wait` + pull.

---

## Чеклист 120fps

- [ ] Тестируешь на `releaseDebug`, не `debug`
- [ ] `@Immutable` / `@Stable` на data class в параметрах Composable
- [ ] Compose compiler reports: `TodoRow` помечен `skippable`, нет `unstable` параметров
- [ ] `key` и `contentType` в `items()`
- [ ] `LazyListPrefetchStrategy(8)` в `rememberLazyListState`
- [ ] `distinctUntilChanged()` ПЕРЕД `flowOn()` — не после
- [ ] SQL-запросы для специфичных списков (не `getAll()` + in-memory filter)
- [ ] `derivedStateOf` для scroll-зависимого состояния
- [ ] `remember(dep)` для вычислений внутри `items {}`
- [ ] `Box + background` вместо `Surface` для простых контейнеров
- [ ] `requestHighRefreshRate()` в MainActivity
- [ ] `enableOnBackInvokedCallback="true"` в манифесте
- [ ] Нет блокирующих операций на main thread
