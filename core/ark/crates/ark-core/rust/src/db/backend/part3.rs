
#[async_trait::async_trait]
impl StorageBackend for SqliteStorageBackend {
    async fn load_entities(&self, vector: &VersionVector) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            // Poison recovery: РµСЃР»Рё earlier panic Р·Р°РїРѕР»СѓС‡РёР» lock, РјС‹ РІСЃС‘
            // СЂР°РІРЅРѕ РјРѕР¶РµРј С‡РёС‚Р°С‚СЊ. Р­С‚Рѕ backend РґР»СЏ load (read-only path),
            // РґР°РЅРЅС‹Рµ РІРЅСѓС‚СЂРё guard'Р° С†РµР»С‹. Р‘РµР· recovery РєР°Р¶РґС‹Р№ РїРѕСЃР»РµРґСѓСЋС‰РёР№
            // sync round РІРѕР·РІСЂР°С‰Р°РµС‚ empty list в†’ multi-device sync silent
            // С„РµР№Р»РёС‚СЃСЏ РЅР°РІСЃРµРіРґР° РїРѕСЃР»Рµ РїРµСЂРІРѕРіРѕ panic'Р° РІ СЌС‚РѕРј РїСЂРѕС†РµСЃСЃРµ.
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::collect_entities_blocking(&guard, &vector, &device_id)
        })
        .await
        .unwrap_or_default()
    }

    async fn load_entities_page(
        &self,
        vector: &VersionVector,
        offset: usize,
        limit: usize,
    ) -> Vec<SyncEntity> {
        let conn = self.conn.clone();
        let vector = vector.clone();
        let device_id = self.device_id();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::collect_entities_page_blocking(&guard, &vector, &device_id, offset, limit)
        })
        .await
        .unwrap_or_default()
    }

    async fn apply_entity(&self, entity: &SyncEntity) -> Result<(), String> {
        let conn = self.conn.clone();
        let entity = entity.clone();
        tokio::task::spawn_blocking(move || {
            // Apply вЂ” write path. Poison recovery acceptable: SQLite
            // transactions atomic, partially-applied state РЅРµ РІРѕР·РјРѕР¶РµРЅ.
            // РђР»СЊС‚РµСЂРЅР°С‚РёРІР° (return Err) РґРµР»Р°РµС‚ sync РЅРµСЂР°Р±РѕС‚РѕСЃРїРѕСЃРѕР±РЅС‹Рј РґРѕ
            // process restart.
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            Self::apply_entity_blocking(&guard, &entity)
        })
        .await
        .map_err(|e| e.to_string())?
    }

    async fn get_kv(&self, key: &str) -> Option<String> {
        let conn = self.conn.clone();
        let key = key.to_string();
        tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            get_sync_kv(&guard, &key).unwrap_or(None)
        })
        .await
        .unwrap_or(None)
    }

    async fn set_kv(&self, key: &str, value: &str) {
        let conn = self.conn.clone();
        let key = key.to_string();
        let value = value.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            let guard = conn.lock().unwrap_or_else(|e| e.into_inner());
            let _ = set_sync_kv(&guard, &key, &value);
        })
        .await;
    }
}

// ---------------------------------------------------------------------------
