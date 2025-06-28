pub use lyserver_messaging_shared::LYServerMessageEvent;

pub fn recv_raw() -> Option<Vec<u8>> {
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
        Some(slice.to_vec())
    }
}

pub fn tx_raw(msg: Vec<u8>) -> usize {
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
        match serde_cbor::from_slice::<LYServerMessageEvent>(&raw) {
            Ok(event) => return Some(event),
            Err(e) => return None
        }
    } else {
        None
    }
}

pub fn tx(msg: &LYServerMessageEvent) -> Result<(), String> {
    let serialized = serde_cbor::to_vec(msg)
        .map_err(|e| format!("Failed to serialize message: {}", e))?;
    
    let len = tx_raw(serialized);
    if len > 1 {
        Err(format!("Failed to send message"))
    } else {
        Ok(())
    }
}