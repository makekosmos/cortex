//! Local-fake-HTTP coverage: a bound `127.0.0.1` hyper server stands in for
//! Open Library / book pages, exercising the real fetch path (DNS pinning,
//! redirects, content-type and size limits) end to end.
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use serde_json::Value;
use tokio::net::TcpListener;

use super::color::{dominant_image_color, image_dimensions};
use super::AppNetworkCtx;

pub(super) fn ctx(data_dir: std::path::PathBuf, origin: Option<String>) -> AppNetworkCtx {
    AppNetworkCtx {
        data_dir,
        allow_private_http: true,
        open_library_origin: origin,
    }
}

pub(super) type Handler = Arc<dyn Fn(&str) -> Response<Full<Bytes>> + Send + Sync>;

pub(super) async fn spawn_server(handler: Handler) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let handler = handler.clone();
            tokio::spawn(async move {
                let service = service_fn(move |request: Request<Incoming>| {
                    let handler = handler.clone();
                    async move { Ok::<_, std::convert::Infallible>(handler(request.uri().path())) }
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            });
        }
    });
    format!("http://127.0.0.1:{port}")
}

pub(super) fn json_response(body: Value) -> Response<Full<Bytes>> {
    Response::builder()
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(body.to_string())))
        .unwrap()
}

pub(super) fn redirect(location: &str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(302)
        .header("location", location)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

#[test]
fn public_network_address_rejects_local_and_reserved() {
    for address in [
        "127.0.0.1",
        "10.0.0.1",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.1.1",
        "::1",
        "::127.0.0.1",
        "fc00::1",
        "fe80::1",
        "2001:db8::1",
    ] {
        assert!(!super::is_public_network_address(address), "{address}");
    }
    assert!(super::is_public_network_address("8.8.8.8"));
    assert!(super::is_public_network_address("2606:4700:4700::1111"));
}

#[test]
fn dominant_color_prefers_chromatic_and_falls_back_for_grayscale() {
    let pixels: Vec<u8> = [
        [132, 82, 38],
        [141, 91, 45],
        [136, 86, 40],
        [138, 88, 42],
        [130, 80, 36],
        [140, 90, 44],
        [110, 110, 110],
        [111, 111, 111],
        [112, 112, 112],
        [113, 113, 113],
        [114, 114, 114],
        [115, 115, 115],
    ]
    .iter()
    .flat_map(|p| [p[0], p[1], p[2], 255])
    .collect();
    assert_eq!(
        dominant_image_color(&pixels, 4).as_deref(),
        Some("rgb(136 86 41)")
    );
    assert_eq!(
        dominant_image_color(&[90, 90, 90, 255], 4).as_deref(),
        Some("rgb(90 90 90)")
    );
    assert_eq!(dominant_image_color(&[0, 0, 0, 0], 4), None);
}

#[test]
fn image_dimensions_reads_png_and_jpeg() {
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend([0, 0, 0, 13]);
    png.extend(b"IHDR");
    png.extend([0, 0, 2, 0, 0, 0, 3, 0]);
    assert_eq!(image_dimensions(&png), Some((512, 768)));
    for marker in [
        0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb, 0xcd, 0xce, 0xcf,
    ] {
        let jpeg = [
            0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, marker, 0, 7, 8, 0x01, 0x2c, 0x02, 0x58,
        ];
        assert_eq!(image_dimensions(&jpeg), Some((600, 300)), "{marker:#x}");
    }
    assert_eq!(image_dimensions(b"\x89PN"), None);
    assert_eq!(image_dimensions(b"GIF89a"), None);
}
