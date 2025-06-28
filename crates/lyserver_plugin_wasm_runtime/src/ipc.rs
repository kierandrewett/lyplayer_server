pub use lyserver_messaging_shared::LYServerMessageEvent;

pub fn recv_raw() -> Option<String> {
    let mut ptr: i32 = 0;
    let mut len: i32 = 0;

    unsafe {
        crate::externs::lyserver_plugin_receive_message(
            &mut ptr as *mut _ as *mut u8,
            &mut len as *mut _ as *mut u8,
        );

        if ptr == 0 || len == 0 {
            return None;
        }

        let slice = std::slice::from_raw_parts(ptr as *const u8, len as usize);
        Some(String::from_utf8_lossy(slice).to_string())
    }
}

pub fn tx_raw(msg: &str) -> usize {
    let mut ret: usize = 0;

    unsafe {
        crate::externs::lyserver_plugin_send_message(
            msg.as_ptr(),
            msg.len(),
            &mut ret as *mut usize,
        );
    }

    ret
}

pub fn recv() -> Option<LYServerMessageEvent> {
    if let Some(raw) = recv_raw() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            return LYServerMessageEvent::try_from(value).ok();
        } else {
            return None;
        }
    } else {
        None
    }
}

pub fn tx(msg: &LYServerMessageEvent) -> Result<(), String> {
    let serialized = serde_json::to_string(msg)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;
    
    let len = tx_raw(&serialized);
    if len > 1 {
        Err(format!("Failed to send message"))
    } else {
        Ok(())
    }
}