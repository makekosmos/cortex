use super::*;

#[test]
fn forced_off_env_disables_gpu_path() {
    // This test is the only one that touches the process-wide slot;
    // the ignored GPU test below bypasses it via GpuLogo::new.
    std::env::set_var("MUNDUS_LOGO_GPU", "off");
    assert!(frame(0.0, 100).is_none());
}

#[test]
fn field_png_decodes_to_canvas_size() {
    let img = image::load_from_memory(FIELD_PNG)
        .expect("field png must decode")
        .to_rgba8();
    assert_eq!(img.dimensions(), (256, 252));
}

#[test]
#[ignore = "requires a GPU/Vulkan adapter; run with --features logo-gpu -- --ignored"]
fn gpu_frame_renders_logo_pixels() {
    let mut g = GpuLogo::new().expect("adapter+device on this box");
    // First tick only submits; the frame arrives via async readback.
    let mut img = None;
    for _ in 0..600 {
        img = g.frame(0.5, 200).expect("frame");
        if img.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let img = img.expect("readback must land within ~6 s");
    let size = img.size(0);
    // Height follows the 256:252 field aspect (200 * 252/256 = 196.9).
    assert_eq!((i32::from(size.width), i32::from(size.height)), (200, 197));
    if let Ok(path) = std::env::var("MUNDUS_LOGO_GPU_DUMP") {
        // Premultiplied BGRA, same bytes the overlay paints.
        std::fs::write(path, img.as_bytes(0).expect("frame bytes")).expect("dump frame");
    }
}
