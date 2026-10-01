async fn serve_asset(
    path: &str,
    package_service: Arc<PackageService>,
    launch_leases: Arc<Mutex<LaunchLeaseRegistry>>,
) -> HttpResponse {
    let Some(rest) = path.strip_prefix("/v1/apps/assets/") else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let Some((token, raw_asset)) = rest.split_once('/') else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    if token.is_empty()
        || token.len() > 128
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return asset_error(StatusCode::NOT_FOUND);
    }
    let Some(asset) = percent_decode(raw_asset) else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let grant = launch_leases
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .asset(token);
    let Some(grant) = grant else {
        return asset_error(StatusCode::NOT_FOUND);
    };
    let bytes = match package_service.read_app_asset(&grant.id, &grant.version, &grant.hash, &asset)
    {
        Ok(bytes) => bytes,
        Err(_) => return asset_error(StatusCode::NOT_FOUND),
    };
    asset_response(&asset, bytes)
}

fn asset_response(asset: &str, bytes: Vec<u8>) -> HttpResponse {
    let html = asset.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("html") || ext.eq_ignore_ascii_case("htm")
    });
    let content_type = mime_type(asset);
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, content_type)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff");
    if html {
        response = response.header(
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        );
    }
    response
        .body(Full::new(Bytes::from(bytes)))
        .unwrap_or_else(|_| asset_error(StatusCode::INTERNAL_SERVER_ERROR))
}

fn asset_error(status: StatusCode) -> HttpResponse {
    Response::builder()
        .status(status)
        .header("cache-control", "no-store")
        .header("referrer-policy", "no-referrer")
        .body(Full::new(Bytes::new()))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::new())))
}

fn percent_decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let input = value.as_bytes();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            if index + 2 >= input.len() {
                return None;
            }
            let high = (input[index + 1] as char).to_digit(16)? as u8;
            let low = (input[index + 2] as char).to_digit(16)? as u8;
            bytes.push((high << 4) | low);
            index += 3;
        } else {
            bytes.push(input[index]);
            index += 1;
        }
    }
    String::from_utf8(bytes).ok()
}

fn mime_type(path: &str) -> &'static str {
    match path
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}