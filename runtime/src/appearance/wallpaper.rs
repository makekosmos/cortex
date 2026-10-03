type WallpaperFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
type WallpaperSampler = Arc<dyn Fn() -> WallpaperFuture + Send + Sync>;

#[derive(Default)]
struct WallpaperCacheState {
    result: Option<Result<String, String>>,
    updated_at: Option<Instant>,
    refreshing: bool,
}

struct WallpaperCache {
    state: Mutex<WallpaperCacheState>,
    sampler: WallpaperSampler,
    ttl: Duration,
}

impl WallpaperCache {
    async fn snapshot(self: &Arc<Self>) -> Result<String, String> {
        let mut state = self.state.lock().await;
        let stale = state
            .updated_at
            .is_none_or(|updated| updated.elapsed() >= self.ttl);
        if stale && !state.refreshing {
            state.refreshing = true;
            let cache = Arc::clone(self);
            tokio::spawn(async move {
                let result = (cache.sampler)().await;
                let mut state = cache.state.lock().await;
                state.result = Some(result);
                state.updated_at = Some(Instant::now());
                state.refreshing = false;
            });
        }
        state
            .result
            .clone()
            .unwrap_or_else(|| Err(WALLPAPER_PENDING.into()))
    }
}

#[cfg(not(target_os = "macos"))]
async fn desktop_wallpaper_accent() -> Result<String, String> {
    Err("desktop wallpaper accent is unsupported on this platform".into())
}

#[cfg(target_os = "macos")]
async fn desktop_wallpaper_accent() -> Result<String, String> {
    use tokio::{process::Command, time::timeout};
    const SCRIPT: &str = concat!(
        "tell application \"System Events\" to get POSIX path of ",
        "(picture of current desktop)"
    );
    let output = timeout(
        Duration::from_secs(5),
        Command::new("/usr/bin/osascript")
            .args(["-e", SCRIPT])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "desktop wallpaper lookup timed out")?
    .map_err(|error| format!("desktop wallpaper lookup failed: {error}"))?;
    if !output.status.success() {
        return Err("desktop wallpaper lookup denied or unavailable".into());
    }
    let image_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    if !image_path.is_file() {
        return Err("desktop wallpaper image is unavailable".into());
    }
    extract_image(&image_path).await
}

#[cfg(target_os = "macos")]
async fn extract_image(path: &Path) -> Result<String, String> {
    use tokio::{process::Command, time::timeout};
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| error.to_string())?;
    if metadata.len() > 64 * 1024 * 1024 {
        return Err("desktop wallpaper image is too large".into());
    }
    // sips handles HEIC desktop pictures as well as JPEG/PNG. Output is a private
    // temporary PNG; no wallpaper file or OS setting is modified.
    let output =
        std::env::temp_dir().join(format!("mundus-wallpaper-{}.png", uuid::Uuid::new_v4()));
    let conversion = timeout(
        Duration::from_secs(5),
        Command::new("/usr/bin/sips")
            .args(["-s", "format", "png", "-Z", "64"])
            .arg(path)
            .arg("--out")
            .arg(&output)
            .kill_on_drop(true)
            .output(),
    )
    .await;
    let result = async {
        let conversion = conversion
            .map_err(|_| "desktop wallpaper decode timed out")?
            .map_err(|error| format!("desktop wallpaper decode failed: {error}"))?;
        if !conversion.status.success() {
            return Err("desktop wallpaper decode failed".into());
        }
        let bytes = tokio::fs::read(&output)
            .await
            .map_err(|error| error.to_string())?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err("desktop wallpaper sample is too large".into());
        }
        tokio::task::spawn_blocking(move || extract_sample(&bytes))
            .await
            .map_err(|error| error.to_string())?
    }
    .await;
    let _ = tokio::fs::remove_file(output).await;
    result
}

#[cfg(target_os = "macos")]
fn extract_sample(bytes: &[u8]) -> Result<String, String> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?
        .to_rgba8();
    let mut bins = vec![(0_f64, [0_f64; 3]); 4096];
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0;
        if a < 128 {
            continue;
        }
        let high = r.max(g).max(b) as f64;
        let low = r.min(g).min(b) as f64;
        let saturation = (high - low) / high.max(1.0);
        let weight = (0.2 + saturation * saturation) * a as f64 / 255.0;
        let index = ((r as usize >> 4) << 8) | ((g as usize >> 4) << 4) | (b as usize >> 4);
        let (count, sums) = &mut bins[index];
        *count += weight;
        for (sum, channel) in sums.iter_mut().zip([r, g, b]) {
            *sum += channel as f64 * weight;
        }
    }
    let (weight, sums) = bins
        .into_iter()
        .max_by(|a, b| a.0.total_cmp(&b.0))
        .ok_or("desktop wallpaper has no visible pixels")?;
    if weight == 0.0 {
        return Err("desktop wallpaper has no visible pixels".into());
    }
    Ok(format!(
        "#{:02X}{:02X}{:02X}",
        (sums[0] / weight).round() as u8,
        (sums[1] / weight).round() as u8,
        (sums[2] / weight).round() as u8
    ))
}
