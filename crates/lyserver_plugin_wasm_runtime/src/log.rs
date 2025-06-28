pub use crate::{log_impl, write, info, warn, error, debug};

#[allow(unused_macros)]
#[macro_export]
macro_rules! log_impl {
    ($func:path, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        unsafe {
            $func(msg.as_ptr(), msg.len());
        }
    }};
}

#[macro_export]
macro_rules! write {
    ($($arg:tt)*) => {
        {
            use lyserver_plugin_wasm_runtime::externs::lyserver_plugin_stdout_write;
            $crate::log_impl!(lyserver_plugin_stdout_write, $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {
        {
            use lyserver_plugin_wasm_runtime::externs::lyserver_plugin_log_info;
            $crate::log_impl!(lyserver_plugin_log_info, $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        {
            use lyserver_plugin_wasm_runtime::externs::lyserver_plugin_log_warn;
            $crate::log_impl!(lyserver_plugin_log_warn, $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        {
            use lyserver_plugin_wasm_runtime::externs::lyserver_plugin_log_error;
            $crate::log_impl!(lyserver_plugin_log_error, $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => {
        {
            use lyserver_plugin_wasm_runtime::externs::lyserver_plugin_log_debug;
            $crate::log_impl!(lyserver_plugin_log_debug, $($arg)*)
        }
    };
}
