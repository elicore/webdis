use libc::{c_char, c_int, c_longlong, c_void, size_t};
use std::ffi::CString;
use std::ptr;

const ERR_UNSUPPORTED: &str =
    "redis-web-hiredis-compat: sync hiredis ABI scaffold loaded; command execution not yet enabled";

pub const REDIS_OK: c_int = 0;
pub const REDIS_ERR: c_int = -1;

pub const REDIS_REPLY_STRING: c_int = 1;
pub const REDIS_REPLY_ARRAY: c_int = 2;
pub const REDIS_REPLY_INTEGER: c_int = 3;
pub const REDIS_REPLY_NIL: c_int = 4;
pub const REDIS_REPLY_STATUS: c_int = 5;
pub const REDIS_REPLY_ERROR: c_int = 6;

#[repr(C)]
pub struct redisContext {
    pub err: c_int,
    pub errstr: [c_char; 128],
    pub fd: c_int,
    pub flags: c_int,
    pub obuf: *mut c_void,
    pub reader: *mut c_void,
    pub connection_type: c_int,
    pub private_data: *mut c_void,
}

#[repr(C)]
pub struct redisReply {
    pub type_: c_int,
    pub integer: c_longlong,
    pub len: size_t,
    pub str_: *mut c_char,
    pub elements: size_t,
    pub element: *mut *mut redisReply,
}

fn write_errstr(ctx: &mut redisContext, msg: &str) {
    ctx.err = REDIS_ERR;
    ctx.errstr.fill(0);
    let bytes = msg.as_bytes();
    let max = ctx.errstr.len().saturating_sub(1);
    let n = bytes.len().min(max);
    for (dst, src) in ctx.errstr.iter_mut().take(n).zip(bytes.iter().take(n)) {
        *dst = *src as c_char;
    }
}

fn new_context_with_error(msg: &str) -> *mut redisContext {
    let mut ctx = Box::new(redisContext {
        err: REDIS_ERR,
        errstr: [0; 128],
        fd: -1,
        flags: 0,
        obuf: ptr::null_mut(),
        reader: ptr::null_mut(),
        connection_type: 0,
        private_data: ptr::null_mut(),
    });
    write_errstr(&mut ctx, msg);
    Box::into_raw(ctx)
}

fn new_error_reply(msg: &str) -> *mut c_void {
    let cmsg = CString::new(msg).unwrap_or_else(|_| CString::new("ERR").unwrap());
    let len = cmsg.as_bytes().len();
    let raw = cmsg.into_raw();

    let reply = Box::new(redisReply {
        type_: REDIS_REPLY_ERROR,
        integer: 0,
        len,
        str_: raw,
        elements: 0,
        element: ptr::null_mut(),
    });

    Box::into_raw(reply) as *mut c_void
}

unsafe fn context_as_mut<'a>(ctx: *mut redisContext) -> Option<&'a mut redisContext> {
    if ctx.is_null() {
        None
    } else {
        Some(&mut *ctx)
    }
}

#[no_mangle]
pub extern "C" fn redisConnect(_ip: *const c_char, _port: c_int) -> *mut redisContext {
    new_context_with_error(ERR_UNSUPPORTED)
}

#[no_mangle]
pub extern "C" fn redisConnectWithTimeout(
    _ip: *const c_char,
    _port: c_int,
    _timeout: c_void,
) -> *mut redisContext {
    new_context_with_error(ERR_UNSUPPORTED)
}

#[no_mangle]
pub extern "C" fn redisConnectUnix(_path: *const c_char) -> *mut redisContext {
    new_context_with_error(ERR_UNSUPPORTED)
}

#[no_mangle]
pub extern "C" fn redisReconnect(ctx: *mut redisContext) -> c_int {
    unsafe {
        if let Some(ctx) = context_as_mut(ctx) {
            write_errstr(ctx, ERR_UNSUPPORTED);
            return REDIS_ERR;
        }
    }
    REDIS_ERR
}

#[no_mangle]
pub extern "C" fn redisFree(ctx: *mut redisContext) {
    if ctx.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(ctx));
    }
}

#[no_mangle]
pub extern "C" fn redisCommand(_ctx: *mut redisContext, _format: *const c_char) -> *mut c_void {
    new_error_reply(ERR_UNSUPPORTED)
}

#[no_mangle]
pub extern "C" fn redisvCommand(
    _ctx: *mut redisContext,
    _format: *const c_char,
    _ap: *mut c_void,
) -> *mut c_void {
    new_error_reply(ERR_UNSUPPORTED)
}

#[no_mangle]
pub extern "C" fn redisCommandArgv(
    _ctx: *mut redisContext,
    _argc: c_int,
    _argv: *const *const c_char,
    _argvlen: *const size_t,
) -> *mut c_void {
    new_error_reply(ERR_UNSUPPORTED)
}

#[no_mangle]
pub extern "C" fn redisAppendCommand(_ctx: *mut redisContext, _format: *const c_char) -> c_int {
    REDIS_ERR
}

#[no_mangle]
pub extern "C" fn redisAppendCommandArgv(
    _ctx: *mut redisContext,
    _argc: c_int,
    _argv: *const *const c_char,
    _argvlen: *const size_t,
) -> c_int {
    REDIS_ERR
}

#[no_mangle]
pub extern "C" fn redisGetReply(ctx: *mut redisContext, reply: *mut *mut c_void) -> c_int {
    unsafe {
        if !reply.is_null() {
            *reply = new_error_reply(ERR_UNSUPPORTED);
        }
        if let Some(ctx) = context_as_mut(ctx) {
            write_errstr(ctx, ERR_UNSUPPORTED);
        }
    }
    REDIS_ERR
}

#[no_mangle]
pub extern "C" fn freeReplyObject(reply: *mut c_void) {
    if reply.is_null() {
        return;
    }

    unsafe {
        let reply = Box::from_raw(reply as *mut redisReply);
        if !reply.str_.is_null() {
            let _ = CString::from_raw(reply.str_);
        }
    }
}

// Async/SSL symbols are intentionally stubbed in v1.
#[no_mangle]
pub extern "C" fn redisAsyncConnect(_ip: *const c_char, _port: c_int) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn redisAsyncConnectUnix(_path: *const c_char) -> *mut c_void {
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn redisInitiateSSLWithContext(_ctx: *mut redisContext, _ssl: *mut c_void) -> c_int {
    REDIS_ERR
}

#[no_mangle]
pub extern "C" fn redisInitOpenSSL() {}
