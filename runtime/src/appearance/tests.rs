use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn manager_client() -> DispatchClient {
    DispatchClient {
        class: Some("manager-gpui".into()),
        ..Default::default()
    }
}

fn client_with_class(class: &str) -> DispatchClient {
    DispatchClient {
        class: Some(class.into()),
        ..Default::default()
    }
}

fn anon_client() -> DispatchClient {
    DispatchClient::default()
}

async fn wait_for_cache(cache: &Arc<WallpaperCache>, refreshing: bool) {
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if cache.state.lock().await.refreshing == refreshing {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

async fn wait_for_calls(calls: &AtomicUsize, expected: usize) {
    tokio::time::timeout(Duration::from_secs(1), async {
        while calls.load(Ordering::SeqCst) != expected {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn wallpaper_cache_is_prompt_singleflight_and_caches_failures() {
    let calls = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(tokio::sync::Notify::new());
    let sampler: WallpaperSampler = Arc::new({
        let calls = calls.clone();
        let gate = gate.clone();
        move || {
            let calls = calls.clone();
            let gate = gate.clone();
            Box::pin(async move {
                let call = calls.fetch_add(1, Ordering::SeqCst);
                gate.notified().await;
                if call == 0 {
                    Err("permission denied".into())
                } else {
                    Ok("#2060A0".into())
                }
            })
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::with_sampler(dir.path().to_path_buf(), sampler, WALLPAPER_TTL);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    store.get(&json!({}), &anon_client()).await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "theme mode must not sample"
    );

    let cache = &store.wallpaper;
    let settings = AppearanceSettings {
        accent_source: "wallpaper".into(),
        ..AppearanceSettings::default()
    };
    let first = store.response(settings).await;
    assert!(first["wallpaper_accent"].is_null());
    assert_eq!(first["wallpaper_error"], WALLPAPER_PENDING);
    wait_for_calls(&calls, 1).await;
    for _ in 0..5 {
        assert_eq!(cache.snapshot().await.unwrap_err(), WALLPAPER_PENDING);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    gate.notify_one();
    wait_for_cache(cache, false).await;
    assert_eq!(cache.snapshot().await.unwrap_err(), "permission denied");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    cache.state.lock().await.updated_at =
        Some(Instant::now() - WALLPAPER_TTL - Duration::from_secs(1));
    assert_eq!(
        cache.snapshot().await.unwrap_err(),
        "permission denied",
        "stale result stays available during refresh"
    );
    wait_for_calls(&calls, 2).await;
    for _ in 0..5 {
        assert_eq!(cache.snapshot().await.unwrap_err(), "permission denied");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    gate.notify_one();
    wait_for_cache(cache, false).await;
    assert_eq!(cache.snapshot().await.unwrap(), "#2060A0");
}

#[tokio::test]
async fn persists_patches_and_assigns_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    assert_eq!(
        store.get(&json!({}), &anon_client()).await.unwrap()["settings"]["revision"],
        0
    );
    assert_eq!(
        store
            .set(
                &json!({"mode":"light", "font_size":12.5}),
                &manager_client()
            )
            .await
            .unwrap()["settings"]["revision"],
        1
    );
    let reopened = AppearanceStore::new(dir.path().to_path_buf());
    let settings = reopened.get(&json!({}), &anon_client()).await.unwrap();
    assert_eq!(settings["settings"]["mode"], "light");
    assert_eq!(settings["settings"]["font_size"], 12.5);
    assert_eq!(
        reopened
            .set(&json!({"follow_apps":true}), &manager_client())
            .await
            .unwrap()["settings"]["revision"],
        2
    );
}

#[tokio::test]
async fn rejects_invalid_patch_without_touching_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    store
        .set(&json!({"mode":"light"}), &manager_client())
        .await
        .unwrap();
    let before = fs::read(&store.path).unwrap();
    for patch in [
        json!({"mode":"wrong"}),
        json!({"font_size":10}),
        json!({"font_size":18.1}),
        json!({"accent_color":"red"}),
        json!({"material":"bogus"}),
        json!({"revision":42}),
        json!({"unknown":true}),
        json!({"font_size":null}),
    ] {
        assert!(
            store.set(&patch, &manager_client()).await.is_err(),
            "{patch}"
        );
        assert_eq!(fs::read(&store.path).unwrap(), before);
    }
}

#[test]
fn font_size_requires_a_finite_number_in_range() {
    let mut settings = AppearanceSettings::default();
    for valid in [11.0, 12.5, 18.0] {
        settings.font_size = valid;
        assert!(validate(&settings).is_ok());
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 10.9, 18.1] {
        settings.font_size = invalid;
        assert!(validate(&settings).is_err());
    }
}

#[tokio::test]
async fn unscoped_set_requires_manager_gpui_client() {
    let dir = tempfile::tempdir().unwrap();
    let store = AppearanceStore::new(dir.path().to_path_buf());
    let before = store.get(&json!({}), &anon_client()).await.unwrap();

    for client in [anon_client(), client_with_class("some-other-app")] {
        assert!(store.set(&json!({"mode": "light"}), &client).await.is_err());
    }
    assert_eq!(store.get(&json!({}), &anon_client()).await.unwrap(), before);

    assert!(store
        .set(&json!({"mode": "light"}), &manager_client())
        .await
        .is_ok());
}

#[test]
fn rpc_registry_lists_only_get_and_set() {
    assert!(is_appearance_operation("appearance.get"));
    assert!(is_appearance_operation("appearance.set"));
    assert!(!is_appearance_operation("appearance.delete"));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn desktop_wallpaper_sampler_extracts_a_real_image() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sample.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([32, 96, 160, 255]))
        .save(&path)
        .unwrap();
    assert_eq!(extract_image(&path).await.unwrap(), "#2060A0");
}
