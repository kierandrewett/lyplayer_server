#[allow(dead_code)]
unsafe extern "C" {
    pub fn lyserver_plugin_stdout_write(ptr: *const u8, len: usize);
    pub fn lyserver_plugin_log_info(ptr: *const u8, len: usize);
    pub fn lyserver_plugin_log_warn(ptr: *const u8, len: usize);
    pub fn lyserver_plugin_log_error(ptr: *const u8, len: usize);
    pub fn lyserver_plugin_log_debug(ptr: *const u8, len: usize);
    pub fn lyserver_plugin_receive_message(ret_ptr: *mut u8, ret_len: *mut u8);
    pub fn lyserver_plugin_send_message(ptr: *const u8, len: usize, ret_ptr: *mut usize);
}