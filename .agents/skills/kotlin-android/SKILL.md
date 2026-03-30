---
name: kotlin-android
description: Android Kotlin + Jetpack Compose best practices for the Delphi app. Use for any Android/Kotlin/Compose/Room/Hilt work in apps/delphi/android/.
---

# Kotlin Android Skill — Delphi App

## Stack

- **Language**: Kotlin (JVM 21)
- **UI**: Jetpack Compose + Material 3 (dark theme only)
- **Architecture**: MVVM + Clean Architecture
- **DI**: Hilt (`@HiltAndroidApp`, `@HiltViewModel`, `@AndroidEntryPoint`)
- **DB**: Room with KSP, Flow<> queries
- **Network**: Ktor Client (CIO engine, WebSockets)
- **Serialization**: Kotlinx Serialization JSON
- **State**: StateFlow + `collectAsStateWithLifecycle()`

## Architecture Rules

### ViewModels
- All ViewModels annotated with `@HiltViewModel` and inherit from `ViewModel`
- Use `SmartListViewModel` as base for all smart list screens (Today, Inbox, etc.)
- Expose UI state as `StateFlow<List<T>>`, never as mutable state
- Launch all coroutines in `viewModelScope`

```kotlin
@HiltViewModel
class InboxViewModel @Inject constructor(
    db: DelphiDatabase,
    syncClient: ArkSyncClient,
) : SmartListViewModel(db, syncClient, SmartList.INBOX)
```

### Room
- All DAO methods are `suspend fun` or return `Flow<>`
- Use `@Upsert` for create/update operations (idempotent)
- Use KSP (not KAPT) for code generation
- Foreign keys with `onDelete = ForeignKey.CASCADE` where appropriate
- Schema exported to `$projectDir/schemas`

```kotlin
@Dao
interface TodoDao {
    @Upsert
    suspend fun upsert(todo: TodoItem)

    @Upsert
    suspend fun upsertAll(todos: List<TodoItem>)

    @Query("SELECT * FROM todos WHERE isTrashed = 0 ORDER BY createdAt DESC")
    fun getAllActive(): Flow<List<TodoItem>>
}
```

### Hilt DI
- `AppModule` — application-scoped singletons (HttpClient, DataStore)
- `DatabaseModule` — Room database and all DAOs
- Inject DAOs via constructor injection in ViewModels, never access DB directly from UI

## Compose Patterns

### Navigation
- Routes defined as `sealed class Screen` with `data object` entries
- Tab navigation: `popUpTo + saveState + restoreState` to prevent stack buildup
- No transition animations for tab switching (`EnterTransition.None`)
- Slide animations for modal screens (settings, project detail)

```kotlin
navController.navigate(screen.route) {
    popUpTo(navController.graph.findStartDestination().id) { saveState = true }
    launchSingleTop = true
    restoreState = true
}
```

### State Observation
Always use `collectAsStateWithLifecycle()` (not `collectAsState()`):

```kotlin
val todos by viewModel.todos.collectAsStateWithLifecycle()
```

### Component Reuse
- `SmartListScaffold` — wrap all list screens (handles FAB, TopAppBar, LazyColumn)
- `TodoRow` — single todo item with priority color and checkbox
- `QuickEntryBar` — bottom input for quick task creation
- `ConnectionIndicator` — sync status dot (green/yellow/red)

Do NOT reimpliment these. Extend via parameters if needed.

### Local UI State
Use `mutableStateOf` only for ephemeral UI state (input text, dialog visibility):

```kotlin
var inputText by remember { mutableStateOf("") }
var showMenu by remember { mutableStateOf(false) }
```

## Data Model Conventions

### Entity IDs
- All IDs are UUIDs as `String` (not auto-increment Long)
- `source_id` in sync events = local entity ID (deduplication key)

### Soft Operations
- Never physically delete completed/cancelled/trashed items
- Use boolean flags: `isCompleted`, `isCancelled`, `isTrashed`
- Logbook shows `isCompleted OR isCancelled`
- Trash shows `isTrashed = true`

### Timestamps
- All timestamps as ISO 8601 UTC strings: `"2025-03-29T14:30:00Z"`
- Use `Instant.now().toString()` for current time

## Sync Integration

### Sending Changes
After any local mutation, immediately send to sync:

```kotlin
fun completeTodo(id: String) {
    viewModelScope.launch {
        val updated = todoDao.getById(id)?.copy(
            isCompleted = true,
            completedAt = Instant.now().toString()
        ) ?: return@launch
        todoDao.upsert(updated)  // ← Local first
        syncClient.sendChange(
            ArkEventMapper.todoToArkChange(updated, "update")
        )  // ← Then sync
    }
}
```

### Receiving Changes
Changes from other devices arrive via `syncClient.onChange{}` callback.
Apply via Room `upsert()` — the Flow will automatically update the UI.

### Offline Queue
If `syncClient.sendChange()` is called while offline, it queues to `PendingChangeDao`.
Outbox flushes automatically on reconnect. No manual handling needed in ViewModels.

## Filtering Logic

All smart list filtering lives in `TodoFilterService`. Add new lists there, not in ViewModels.

| SmartList | Filter |
|-----------|--------|
| TODAY | `isToday=true` OR `scheduledDate=today` |
| INBOX | `projectId=null` AND `isSomeday=false` |
| UPCOMING | `scheduledDate > today` |
| LOGBOOK | `isCompleted=true` OR `isCancelled=true` |
| TRASH | `isTrashed=true` |

## File Structure

```
data/
  model/        — Room @Entity, enums
  db/           — DAOs, DelphiDatabase
  sync/         — ArkSyncClient, ArkEventMapper, HLC, Pairing
ui/
  theme/        — Colors, Typography, Theme (dark only)
  components/   — Reusable Compose components
  screens/      — SmartListViewModel base + screen-specific
  navigation/   — NavGraph, sealed Screen routes
di/             — AppModule, DatabaseModule
domain/filter/  — TodoFilterService
```

## Build Notes

- `compileSdk` / `targetSdk`: 36, `minSdk`: 28
- Java: `VERSION_21`, `jvmTarget = "21"`
- Use KSP, never KAPT
- CameraX + ML Kit used for QR scanner — don't add duplicate camera deps

## Self-Check Before Finishing

- [ ] No direct DB access from Composables or UI — only via ViewModel
- [ ] StateFlow exposed, not MutableStateFlow
- [ ] `collectAsStateWithLifecycle()` used (not `collectAsState()`)
- [ ] Local mutation happens before sync call
- [ ] Reused `SmartListScaffold`, `TodoRow`, other shared components
- [ ] Soft deletes used (flags), not physical `DELETE`
- [ ] Timestamps in ISO 8601 UTC
- [ ] Filtering logic added to `TodoFilterService`, not scattered
