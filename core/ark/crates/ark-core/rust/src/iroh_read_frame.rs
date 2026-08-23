async fn write_frame(send: &mut iroh::endpoint::SendStream, payload: &[u8]) -> Result<(), String> {
    let len = u32::try_from(payload.len())
        .map_err(|_| "iroh transport: frame payload too large".to_string())?;
    send.write_all(&len.to_be_bytes())
        .await
        .map_err(|e| format!("iroh transport: write frame length failed: {e}"))?;
    send.write_all(payload)
        .await
        .map_err(|e| format!("iroh transport: write frame payload failed: {e}"))?;
    Ok(())
}

async fn read_frame(recv: &mut iroh::endpoint::RecvStream) -> Result<String, String> {
    let mut len_buf = [0u8; 4];
    recv.read_exact(&mut len_buf)
        .await
        .map_err(|e| format!("iroh transport: read frame length failed: {e}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_LEN {
        return Err(format!(
            "iroh transport: frame length {len} exceeds max {MAX_FRAME_LEN}"
        ));
    }

    let mut payload = vec![0u8; len];
    recv.read_exact(&mut payload)
        .await
        .map_err(|e| format!("iroh transport: read frame payload failed: {e}"))?;

    String::from_utf8(payload).map_err(|e| format!("iroh transport: frame payload not utf8: {e}"))
}

// ---------------------------------------------------------------------------
