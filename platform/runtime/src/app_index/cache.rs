// Cache — placeholder. In-memory cache живёт в `AppIndex.cache` напрямую
// как `Arc<RwLock<Vec<App>>>`, отдельный модуль не нужен. Файл оставлен
// чтобы зарезервировать имя на случай добавления prefix-index / trie позже.
