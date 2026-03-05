use libc::{c_char, c_double, c_int, c_longlong, c_void, size_t};
use std::collections::HashMap;
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::slice;
use std::str;
use std::sync::{Mutex, OnceLock};

const ERR_UNSUPPORTED: &str =
    "redis-web-hiredis-compat: sync hiredis ABI scaffold loaded; command execution not yet enabled";

pub const REDIS_OK: c_int = 0;
pub const REDIS_ERR: c_int = -1;

pub const REDIS_ERR_IO: c_int = 1;
pub const REDIS_ERR_OTHER: c_int = 2;
pub const REDIS_ERR_EOF: c_int = 3;
pub const REDIS_ERR_PROTOCOL: c_int = 4;
pub const REDIS_ERR_OOM: c_int = 5;
pub const REDIS_ERR_TIMEOUT: c_int = 6;

pub const REDIS_REPLY_STRING: c_int = 1;
pub const REDIS_REPLY_ARRAY: c_int = 2;
pub const REDIS_REPLY_INTEGER: c_int = 3;
pub const REDIS_REPLY_NIL: c_int = 4;
pub const REDIS_REPLY_STATUS: c_int = 5;
pub const REDIS_REPLY_ERROR: c_int = 6;
pub const REDIS_REPLY_DOUBLE: c_int = 7;
pub const REDIS_REPLY_BOOL: c_int = 8;
pub const REDIS_REPLY_MAP: c_int = 9;
pub const REDIS_REPLY_SET: c_int = 10;
pub const REDIS_REPLY_ATTR: c_int = 11;
pub const REDIS_REPLY_PUSH: c_int = 12;
pub const REDIS_REPLY_BIGNUM: c_int = 13;
pub const REDIS_REPLY_VERB: c_int = 14;

pub const REDIS_READER_MAX_BUF: size_t = 1024 * 16;
pub const REDIS_READER_MAX_ARRAY_ELEMENTS: c_longlong = ((1u64 << 32) - 1) as c_longlong;

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
    pub r#type: c_int,
    pub integer: c_longlong,
    pub dval: c_double,
    pub len: size_t,
    pub str_: *mut c_char,
    pub vtype: [c_char; 4],
    pub elements: size_t,
    pub element: *mut *mut redisReply,
}

#[repr(C)]
pub struct redisReadTask {
    pub r#type: c_int,
    pub elements: c_longlong,
    pub idx: c_int,
    pub obj: *mut c_void,
    pub parent: *mut redisReadTask,
    pub privdata: *mut c_void,
}

#[repr(C)]
#[allow(non_snake_case)]
pub struct redisReplyObjectFunctions {
    pub createString:
        Option<unsafe extern "C" fn(*const redisReadTask, *mut c_char, size_t) -> *mut c_void>,
    pub createArray:
        Option<unsafe extern "C" fn(*const redisReadTask, size_t) -> *mut c_void>,
    pub createInteger:
        Option<unsafe extern "C" fn(*const redisReadTask, c_longlong) -> *mut c_void>,
    pub createDouble:
        Option<unsafe extern "C" fn(*const redisReadTask, c_double, *mut c_char, size_t) -> *mut c_void>,
    pub createNil: Option<unsafe extern "C" fn(*const redisReadTask) -> *mut c_void>,
    pub createBool: Option<unsafe extern "C" fn(*const redisReadTask, c_int) -> *mut c_void>,
    pub freeObject: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[repr(C)]
pub struct redisReader {
    pub err: c_int,
    pub errstr: [c_char; 128],
    pub buf: *mut c_char,
    pub pos: size_t,
    pub len: size_t,
    pub maxbuf: size_t,
    pub maxelements: c_longlong,
    pub task: *mut *mut redisReadTask,
    pub tasks: c_int,
    pub ridx: c_int,
    pub reply: *mut c_void,
    pub fn_: *mut redisReplyObjectFunctions,
    pub privdata: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
#[allow(non_snake_case)]
pub struct hiredisAllocFuncs {
    pub mallocFn: Option<unsafe extern "C" fn(size_t) -> *mut c_void>,
    pub callocFn: Option<unsafe extern "C" fn(size_t, size_t) -> *mut c_void>,
    pub reallocFn: Option<unsafe extern "C" fn(*mut c_void, size_t) -> *mut c_void>,
    pub strdupFn: Option<unsafe extern "C" fn(*const c_char) -> *mut c_char>,
    pub freeFn: Option<unsafe extern "C" fn(*mut c_void)>,
}

#[derive(Default)]
struct ReaderState {
    buffer: Vec<u8>,
}

#[repr(C)]
struct SdsHeader {
    len: size_t,
    cap: size_t,
}

fn reader_states() -> &'static Mutex<HashMap<usize, ReaderState>> {
    static STATES: OnceLock<Mutex<HashMap<usize, ReaderState>>> = OnceLock::new();
    STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe extern "C" fn default_malloc(size: size_t) -> *mut c_void {
    libc::malloc(size)
}

unsafe extern "C" fn default_calloc(nmemb: size_t, size: size_t) -> *mut c_void {
    libc::calloc(nmemb, size)
}

unsafe extern "C" fn default_realloc(ptr_: *mut c_void, size: size_t) -> *mut c_void {
    libc::realloc(ptr_, size)
}

unsafe extern "C" fn default_strdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    libc::strdup(s)
}

unsafe extern "C" fn default_free(ptr_: *mut c_void) {
    libc::free(ptr_)
}

#[no_mangle]
#[allow(non_upper_case_globals)]
pub static mut hiredisAllocFns: hiredisAllocFuncs = hiredisAllocFuncs {
    mallocFn: Some(default_malloc),
    callocFn: Some(default_calloc),
    reallocFn: Some(default_realloc),
    strdupFn: Some(default_strdup),
    freeFn: Some(default_free),
};

unsafe fn alloc_malloc(size: size_t) -> *mut c_void {
    match hiredisAllocFns.mallocFn {
        Some(f) => f(size),
        None => ptr::null_mut(),
    }
}

unsafe fn alloc_calloc(nmemb: size_t, size: size_t) -> *mut c_void {
    match hiredisAllocFns.callocFn {
        Some(f) => f(nmemb, size),
        None => ptr::null_mut(),
    }
}

unsafe fn alloc_realloc(ptr_: *mut c_void, size: size_t) -> *mut c_void {
    match hiredisAllocFns.reallocFn {
        Some(f) => f(ptr_, size),
        None => ptr::null_mut(),
    }
}

unsafe fn alloc_free(ptr_: *mut c_void) {
    if let Some(f) = hiredisAllocFns.freeFn {
        f(ptr_);
    }
}

unsafe fn alloc_strdup(s: *const c_char) -> *mut c_char {
    match hiredisAllocFns.strdupFn {
        Some(f) => f(s),
        None => ptr::null_mut(),
    }
}

unsafe fn write_errstr_raw(buf: &mut [c_char], msg: &str) {
    buf.fill(0);
    let bytes = msg.as_bytes();
    let max = buf.len().saturating_sub(1);
    let n = bytes.len().min(max);
    for (dst, src) in buf.iter_mut().take(n).zip(bytes.iter().take(n)) {
        *dst = *src as c_char;
    }
}

fn write_errstr(ctx: &mut redisContext, msg: &str) {
    ctx.err = REDIS_ERR;
    unsafe {
        write_errstr_raw(&mut ctx.errstr, msg);
    }
}

unsafe fn set_reader_error(reader: *mut redisReader, code: c_int, msg: &str) {
    if reader.is_null() {
        return;
    }
    (*reader).err = code;
    write_errstr_raw(&mut (*reader).errstr, msg);
}

unsafe fn clear_reader_error(reader: *mut redisReader) {
    if reader.is_null() {
        return;
    }
    (*reader).err = REDIS_OK;
    (*reader).errstr.fill(0);
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

unsafe fn alloc_reply() -> *mut redisReply {
    let mem = alloc_calloc(1, mem::size_of::<redisReply>()) as *mut redisReply;
    mem
}

unsafe fn alloc_c_string(bytes: &[u8]) -> *mut c_char {
    let mem = alloc_malloc(bytes.len() + 1) as *mut c_char;
    if mem.is_null() {
        return ptr::null_mut();
    }
    ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, mem, bytes.len());
    *mem.add(bytes.len()) = 0;
    mem
}

unsafe fn default_parentize(task: *const redisReadTask, child: *mut redisReply) -> *mut c_void {
    if task.is_null() || (*task).parent.is_null() {
        return child as *mut c_void;
    }

    let parent_task = (*task).parent;
    if parent_task.is_null() || (*parent_task).obj.is_null() {
        return child as *mut c_void;
    }

    let parent_reply = (*parent_task).obj as *mut redisReply;
    if parent_reply.is_null() {
        return child as *mut c_void;
    }

    if (*parent_reply).element.is_null() {
        return child as *mut c_void;
    }

    let idx = (*task).idx;
    if idx >= 0 && (idx as size_t) < (*parent_reply).elements {
        *(*parent_reply).element.add(idx as usize) = child;
    }

    child as *mut c_void
}

unsafe fn default_create_string(task: *const redisReadTask, s: &[u8], ty: c_int) -> *mut c_void {
    let reply = alloc_reply();
    if reply.is_null() {
        return ptr::null_mut();
    }

    (*reply).r#type = ty;
    (*reply).len = s.len();
    (*reply).str_ = alloc_c_string(s);
    if (*reply).str_.is_null() && !s.is_empty() {
        alloc_free(reply as *mut c_void);
        return ptr::null_mut();
    }

    if ty == REDIS_REPLY_VERB && s.len() >= 3 {
        let n = s.len().min(3);
        for i in 0..n {
            (*reply).vtype[i] = s[i] as c_char;
        }
        if n < 4 {
            (*reply).vtype[n] = 0;
        }
    }

    default_parentize(task, reply)
}

unsafe fn default_create_array(task: *const redisReadTask, elements: size_t, ty: c_int) -> *mut c_void {
    let reply = alloc_reply();
    if reply.is_null() {
        return ptr::null_mut();
    }

    (*reply).r#type = ty;
    (*reply).elements = elements;
    if elements > 0 {
        let ptr = alloc_calloc(elements, mem::size_of::<*mut redisReply>()) as *mut *mut redisReply;
        if ptr.is_null() {
            alloc_free(reply as *mut c_void);
            return ptr::null_mut();
        }
        (*reply).element = ptr;
    }

    if !task.is_null() {
        (*(task as *mut redisReadTask)).obj = reply as *mut c_void;
    }

    default_parentize(task, reply)
}

unsafe fn default_create_integer(task: *const redisReadTask, value: c_longlong) -> *mut c_void {
    let reply = alloc_reply();
    if reply.is_null() {
        return ptr::null_mut();
    }
    (*reply).r#type = REDIS_REPLY_INTEGER;
    (*reply).integer = value;
    default_parentize(task, reply)
}

unsafe fn default_create_double(task: *const redisReadTask, value: c_double, s: &[u8]) -> *mut c_void {
    let reply = alloc_reply();
    if reply.is_null() {
        return ptr::null_mut();
    }
    (*reply).r#type = REDIS_REPLY_DOUBLE;
    (*reply).dval = value;
    (*reply).len = s.len();
    (*reply).str_ = alloc_c_string(s);
    default_parentize(task, reply)
}

unsafe fn default_create_nil(task: *const redisReadTask) -> *mut c_void {
    let reply = alloc_reply();
    if reply.is_null() {
        return ptr::null_mut();
    }
    (*reply).r#type = REDIS_REPLY_NIL;
    default_parentize(task, reply)
}

unsafe fn default_create_bool(task: *const redisReadTask, b: c_int) -> *mut c_void {
    let reply = alloc_reply();
    if reply.is_null() {
        return ptr::null_mut();
    }
    (*reply).r#type = REDIS_REPLY_BOOL;
    (*reply).integer = if b == 0 { 0 } else { 1 };
    default_parentize(task, reply)
}

unsafe fn call_create_string(task: *mut redisReadTask, reader: *mut redisReader, s: &[u8]) -> *mut c_void {
    if !reader.is_null() && !(*reader).fn_.is_null() {
        if let Some(cb) = (*(*reader).fn_).createString {
            let mut tmp = s.to_vec();
            return cb(task as *const redisReadTask, tmp.as_mut_ptr() as *mut c_char, tmp.len());
        }
    }

    let ty = if task.is_null() { REDIS_REPLY_STRING } else { (*task).r#type };
    default_create_string(task as *const redisReadTask, s, ty)
}

unsafe fn call_create_array(task: *mut redisReadTask, reader: *mut redisReader, elements: size_t) -> *mut c_void {
    if !reader.is_null() && !(*reader).fn_.is_null() {
        if let Some(cb) = (*(*reader).fn_).createArray {
            return cb(task as *const redisReadTask, elements);
        }
    }

    let ty = if task.is_null() { REDIS_REPLY_ARRAY } else { (*task).r#type };
    default_create_array(task as *const redisReadTask, elements, ty)
}

unsafe fn call_create_integer(
    task: *mut redisReadTask,
    reader: *mut redisReader,
    value: c_longlong,
) -> *mut c_void {
    if !reader.is_null() && !(*reader).fn_.is_null() {
        if let Some(cb) = (*(*reader).fn_).createInteger {
            return cb(task as *const redisReadTask, value);
        }
    }

    default_create_integer(task as *const redisReadTask, value)
}

unsafe fn call_create_double(
    task: *mut redisReadTask,
    reader: *mut redisReader,
    value: c_double,
    s: &[u8],
) -> *mut c_void {
    if !reader.is_null() && !(*reader).fn_.is_null() {
        if let Some(cb) = (*(*reader).fn_).createDouble {
            let mut tmp = s.to_vec();
            return cb(
                task as *const redisReadTask,
                value,
                tmp.as_mut_ptr() as *mut c_char,
                tmp.len(),
            );
        }
    }

    default_create_double(task as *const redisReadTask, value, s)
}

unsafe fn call_create_nil(task: *mut redisReadTask, reader: *mut redisReader) -> *mut c_void {
    if !reader.is_null() && !(*reader).fn_.is_null() {
        if let Some(cb) = (*(*reader).fn_).createNil {
            return cb(task as *const redisReadTask);
        }
    }

    default_create_nil(task as *const redisReadTask)
}

unsafe fn call_create_bool(task: *mut redisReadTask, reader: *mut redisReader, b: c_int) -> *mut c_void {
    if !reader.is_null() && !(*reader).fn_.is_null() {
        if let Some(cb) = (*(*reader).fn_).createBool {
            return cb(task as *const redisReadTask, b);
        }
    }

    default_create_bool(task as *const redisReadTask, b)
}

enum ParseError {
    Incomplete,
    Protocol(String),
}

fn find_crlf(data: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 1 < data.len() {
        if data[i] == b'\r' && data[i + 1] == b'\n' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn parse_i64_ascii(bytes: &[u8]) -> Result<i64, ParseError> {
    let s = str::from_utf8(bytes).map_err(|_| ParseError::Protocol("invalid integer".to_string()))?;
    s.parse::<i64>()
        .map_err(|_| ParseError::Protocol("invalid integer".to_string()))
}

unsafe fn parse_and_build(
    reader: *mut redisReader,
    data: &[u8],
    offset: usize,
    parent: *mut redisReadTask,
    idx: c_int,
) -> Result<(*mut c_void, usize), ParseError> {
    if offset >= data.len() {
        return Err(ParseError::Incomplete);
    }

    let prefix = data[offset];
    match prefix {
        b'+' | b'-' | b':' | b',' | b'(' => {
            let line_end = find_crlf(data, offset + 1).ok_or(ParseError::Incomplete)?;
            let line = &data[offset + 1..line_end];
            let consumed = (line_end + 2) - offset;

            let mut task = redisReadTask {
                r#type: match prefix {
                    b'+' => REDIS_REPLY_STATUS,
                    b'-' => REDIS_REPLY_ERROR,
                    b':' => REDIS_REPLY_INTEGER,
                    b',' => REDIS_REPLY_DOUBLE,
                    b'(' => REDIS_REPLY_BIGNUM,
                    _ => REDIS_REPLY_STRING,
                },
                elements: -1,
                idx,
                obj: ptr::null_mut(),
                parent,
                privdata: if reader.is_null() { ptr::null_mut() } else { (*reader).privdata },
            };

            let obj = match prefix {
                b'+' | b'-' | b'(' => call_create_string(&mut task, reader, line),
                b':' => {
                    let value = parse_i64_ascii(line)?;
                    call_create_integer(&mut task, reader, value as c_longlong)
                }
                b',' => {
                    let s = str::from_utf8(line).map_err(|_| ParseError::Protocol("invalid double".to_string()))?;
                    let value = s
                        .parse::<f64>()
                        .map_err(|_| ParseError::Protocol("invalid double".to_string()))?;
                    call_create_double(&mut task, reader, value, line)
                }
                _ => ptr::null_mut(),
            };

            if obj.is_null() {
                return Err(ParseError::Protocol("failed to create reply object".to_string()));
            }

            Ok((obj, consumed))
        }
        b'$' | b'=' => {
            let line_end = find_crlf(data, offset + 1).ok_or(ParseError::Incomplete)?;
            let line = &data[offset + 1..line_end];
            let count = parse_i64_ascii(line)?;
            let header = (line_end + 2) - offset;

            let mut task = redisReadTask {
                r#type: if prefix == b'=' {
                    REDIS_REPLY_VERB
                } else {
                    REDIS_REPLY_STRING
                },
                elements: -1,
                idx,
                obj: ptr::null_mut(),
                parent,
                privdata: if reader.is_null() { ptr::null_mut() } else { (*reader).privdata },
            };

            if count < 0 {
                let obj = call_create_nil(&mut task, reader);
                if obj.is_null() {
                    return Err(ParseError::Protocol("failed to create nil object".to_string()));
                }
                return Ok((obj, header));
            }

            let len = count as usize;
            let start = line_end + 2;
            let end = start + len;
            if end + 2 > data.len() {
                return Err(ParseError::Incomplete);
            }
            if data[end] != b'\r' || data[end + 1] != b'\n' {
                return Err(ParseError::Protocol("invalid bulk string terminator".to_string()));
            }

            let payload = &data[start..end];
            let obj = call_create_string(&mut task, reader, payload);
            if obj.is_null() {
                return Err(ParseError::Protocol("failed to create string object".to_string()));
            }

            Ok((obj, header + len + 2))
        }
        b'#' => {
            if offset + 3 >= data.len() {
                return Err(ParseError::Incomplete);
            }
            if data[offset + 2] != b'\r' || data[offset + 3] != b'\n' {
                return Err(ParseError::Protocol("invalid bool frame".to_string()));
            }

            let value = match data[offset + 1] {
                b't' | b'T' => 1,
                b'f' | b'F' => 0,
                _ => return Err(ParseError::Protocol("invalid bool value".to_string())),
            };

            let mut task = redisReadTask {
                r#type: REDIS_REPLY_BOOL,
                elements: -1,
                idx,
                obj: ptr::null_mut(),
                parent,
                privdata: if reader.is_null() { ptr::null_mut() } else { (*reader).privdata },
            };

            let obj = call_create_bool(&mut task, reader, value);
            if obj.is_null() {
                return Err(ParseError::Protocol("failed to create bool object".to_string()));
            }

            Ok((obj, 4))
        }
        b'_' => {
            if offset + 2 >= data.len() {
                return Err(ParseError::Incomplete);
            }
            if data[offset + 1] != b'\r' || data[offset + 2] != b'\n' {
                return Err(ParseError::Protocol("invalid null frame".to_string()));
            }

            let mut task = redisReadTask {
                r#type: REDIS_REPLY_NIL,
                elements: -1,
                idx,
                obj: ptr::null_mut(),
                parent,
                privdata: if reader.is_null() { ptr::null_mut() } else { (*reader).privdata },
            };

            let obj = call_create_nil(&mut task, reader);
            if obj.is_null() {
                return Err(ParseError::Protocol("failed to create nil object".to_string()));
            }

            Ok((obj, 3))
        }
        b'*' | b'~' | b'>' | b'|' | b'%' => {
            let line_end = find_crlf(data, offset + 1).ok_or(ParseError::Incomplete)?;
            let line = &data[offset + 1..line_end];
            let count = parse_i64_ascii(line)?;
            let header = (line_end + 2) - offset;

            if count < 0 {
                let mut nil_task = redisReadTask {
                    r#type: REDIS_REPLY_NIL,
                    elements: -1,
                    idx,
                    obj: ptr::null_mut(),
                    parent,
                    privdata: if reader.is_null() { ptr::null_mut() } else { (*reader).privdata },
                };
                let obj = call_create_nil(&mut nil_task, reader);
                if obj.is_null() {
                    return Err(ParseError::Protocol("failed to create nil object".to_string()));
                }
                return Ok((obj, header));
            }

            if count as c_longlong > REDIS_READER_MAX_ARRAY_ELEMENTS {
                return Err(ParseError::Protocol("array element count exceeds limit".to_string()));
            }

            let frame_type = match prefix {
                b'*' => REDIS_REPLY_ARRAY,
                b'~' => REDIS_REPLY_SET,
                b'>' => REDIS_REPLY_PUSH,
                b'|' => REDIS_REPLY_ATTR,
                b'%' => REDIS_REPLY_MAP,
                _ => REDIS_REPLY_ARRAY,
            };

            let logical_count = count as usize;
            let child_count = if prefix == b'%' || prefix == b'|' {
                logical_count.saturating_mul(2)
            } else {
                logical_count
            };

            let mut task = redisReadTask {
                r#type: frame_type,
                elements: logical_count as c_longlong,
                idx,
                obj: ptr::null_mut(),
                parent,
                privdata: if reader.is_null() { ptr::null_mut() } else { (*reader).privdata },
            };

            let obj = call_create_array(&mut task, reader, logical_count);
            if obj.is_null() {
                return Err(ParseError::Protocol("failed to create aggregate object".to_string()));
            }
            task.obj = obj;

            let mut consumed = header;
            for child_idx in 0..child_count {
                let (_, child_consumed) = parse_and_build(
                    reader,
                    data,
                    offset + consumed,
                    &mut task as *mut redisReadTask,
                    child_idx as c_int,
                )?;
                consumed += child_consumed;
            }

            Ok((obj, consumed))
        }
        _ => {
            let ch = if prefix.is_ascii_graphic() || prefix == b' ' {
                (prefix as char).to_string()
            } else {
                format!("\\x{:02x}", prefix)
            };
            Err(ParseError::Protocol(format!(
                "Protocol error, got \"{}\" as reply type byte",
                ch
            )))
        }
    }
}

unsafe fn update_reader_metrics(reader: *mut redisReader, state: &ReaderState) {
    if reader.is_null() {
        return;
    }
    (*reader).buf = ptr::null_mut();
    (*reader).pos = 0;
    (*reader).len = state.buffer.len();
}

unsafe fn context_as_mut<'a>(ctx: *mut redisContext) -> Option<&'a mut redisContext> {
    if ctx.is_null() {
        None
    } else {
        Some(&mut *ctx)
    }
}

unsafe fn free_reply_recursive(reply: *mut redisReply) {
    if reply.is_null() {
        return;
    }

    if !(*reply).element.is_null() {
        for i in 0..(*reply).elements {
            let child = *(*reply).element.add(i);
            free_reply_recursive(child);
        }
        alloc_free((*reply).element as *mut c_void);
    }

    if !(*reply).str_.is_null() {
        alloc_free((*reply).str_ as *mut c_void);
    }

    alloc_free(reply as *mut c_void);
}

fn new_error_reply(msg: &str) -> *mut c_void {
    let cmsg = CString::new(msg).unwrap_or_else(|_| CString::new("ERR").unwrap());
    let bytes = cmsg.as_bytes();

    unsafe {
        let reply = alloc_reply();
        if reply.is_null() {
            return ptr::null_mut();
        }

        (*reply).r#type = REDIS_REPLY_ERROR;
        (*reply).len = bytes.len();
        (*reply).str_ = alloc_c_string(bytes);

        if (*reply).str_.is_null() && !bytes.is_empty() {
            alloc_free(reply as *mut c_void);
            return ptr::null_mut();
        }

        reply as *mut c_void
    }
}

unsafe fn build_resp_command(argc: c_int, argv: *const *const c_char, argvlen: *const size_t) -> Option<Vec<u8>> {
    if argc < 0 {
        return None;
    }

    let argc_usize = argc as usize;
    let mut out = Vec::new();
    out.extend_from_slice(format!("*{}\r\n", argc_usize).as_bytes());

    for i in 0..argc_usize {
        let arg_ptr = *argv.add(i);
        if arg_ptr.is_null() {
            return None;
        }

        let len = if argvlen.is_null() {
            libc::strlen(arg_ptr)
        } else {
            *argvlen.add(i)
        };

        let bytes = slice::from_raw_parts(arg_ptr as *const u8, len);
        out.extend_from_slice(format!("${}\r\n", len).as_bytes());
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\r\n");
    }

    Some(out)
}

unsafe fn sds_header_ptr(s: *mut c_char) -> *mut SdsHeader {
    (s as *mut u8).sub(mem::size_of::<SdsHeader>()) as *mut SdsHeader
}

unsafe fn sds_alloc_with_cap(cap: size_t) -> *mut c_char {
    let total = mem::size_of::<SdsHeader>() + cap + 1;
    let raw = alloc_malloc(total) as *mut u8;
    if raw.is_null() {
        return ptr::null_mut();
    }

    let hdr = raw as *mut SdsHeader;
    (*hdr).len = 0;
    (*hdr).cap = cap;

    let data = raw.add(mem::size_of::<SdsHeader>()) as *mut c_char;
    *data = 0;
    data
}

unsafe fn sds_len(s: *const c_char) -> size_t {
    if s.is_null() {
        return 0;
    }
    (*sds_header_ptr(s as *mut c_char)).len
}

unsafe fn sds_cap(s: *const c_char) -> size_t {
    if s.is_null() {
        return 0;
    }
    (*sds_header_ptr(s as *mut c_char)).cap
}

unsafe fn sds_set_len(s: *mut c_char, len: size_t) {
    let hdr = sds_header_ptr(s);
    (*hdr).len = len;
    *s.add(len) = 0;
}

unsafe fn sds_ensure_cap(s: *mut c_char, cap: size_t) -> *mut c_char {
    if s.is_null() {
        return sds_alloc_with_cap(cap);
    }

    if sds_cap(s) >= cap {
        return s;
    }

    let hdr = sds_header_ptr(s);
    let total = mem::size_of::<SdsHeader>() + cap + 1;
    let raw = alloc_realloc(hdr as *mut c_void, total) as *mut u8;
    if raw.is_null() {
        return ptr::null_mut();
    }

    let new_hdr = raw as *mut SdsHeader;
    (*new_hdr).cap = cap;
    raw.add(mem::size_of::<SdsHeader>()) as *mut c_char
}

#[no_mangle]
pub unsafe extern "C" fn hiredisSetAllocators(ha: *mut hiredisAllocFuncs) -> hiredisAllocFuncs {
    let prev = hiredisAllocFns;
    if !ha.is_null() {
        hiredisAllocFns = *ha;
    }
    prev
}

#[no_mangle]
pub unsafe extern "C" fn hiredisResetAllocators() {
    hiredisAllocFns = hiredisAllocFuncs {
        mallocFn: Some(default_malloc),
        callocFn: Some(default_calloc),
        reallocFn: Some(default_realloc),
        strdupFn: Some(default_strdup),
        freeFn: Some(default_free),
    };
}

#[no_mangle]
pub unsafe extern "C" fn hi_malloc(size: size_t) -> *mut c_void {
    alloc_malloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn hi_calloc(nmemb: size_t, size: size_t) -> *mut c_void {
    alloc_calloc(nmemb, size)
}

#[no_mangle]
pub unsafe extern "C" fn hi_realloc(ptr_: *mut c_void, size: size_t) -> *mut c_void {
    alloc_realloc(ptr_, size)
}

#[no_mangle]
pub unsafe extern "C" fn hi_strdup(s: *const c_char) -> *mut c_char {
    alloc_strdup(s)
}

#[no_mangle]
pub unsafe extern "C" fn hi_free(ptr_: *mut c_void) {
    alloc_free(ptr_)
}

#[no_mangle]
pub unsafe extern "C" fn sdsnewlen(init: *const c_void, initlen: size_t) -> *mut c_char {
    let s = sds_alloc_with_cap(initlen);
    if s.is_null() {
        return ptr::null_mut();
    }

    if !init.is_null() && initlen > 0 {
        ptr::copy_nonoverlapping(init as *const c_char, s, initlen);
    }
    sds_set_len(s, initlen);
    s
}

#[no_mangle]
pub unsafe extern "C" fn sdsnew(init: *const c_char) -> *mut c_char {
    if init.is_null() {
        return sdsempty();
    }
    sdsnewlen(init as *const c_void, libc::strlen(init))
}

#[no_mangle]
pub unsafe extern "C" fn sdsempty() -> *mut c_char {
    sdsnewlen(ptr::null(), 0)
}

#[no_mangle]
pub unsafe extern "C" fn sdsdup(s: *const c_char) -> *mut c_char {
    if s.is_null() {
        return ptr::null_mut();
    }
    sdsnewlen(s as *const c_void, sds_len(s))
}

#[no_mangle]
pub unsafe extern "C" fn sdsfree(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    alloc_free(sds_header_ptr(s) as *mut c_void)
}

#[no_mangle]
pub unsafe extern "C" fn sds_free(ptr_: *mut c_void) {
    sdsfree(ptr_ as *mut c_char)
}

#[no_mangle]
pub unsafe extern "C" fn sds_malloc(size: size_t) -> *mut c_void {
    alloc_malloc(size)
}

#[no_mangle]
pub unsafe extern "C" fn sds_realloc(ptr_: *mut c_void, size: size_t) -> *mut c_void {
    alloc_realloc(ptr_, size)
}

#[no_mangle]
pub unsafe extern "C" fn sdscpylen(s: *mut c_char, t: *const c_char, len: size_t) -> *mut c_char {
    if t.is_null() && len > 0 {
        return ptr::null_mut();
    }

    let out = sds_ensure_cap(s, len);
    if out.is_null() {
        return ptr::null_mut();
    }

    if len > 0 {
        ptr::copy_nonoverlapping(t, out, len);
    }
    sds_set_len(out, len);
    out
}

#[no_mangle]
pub unsafe extern "C" fn sdscpy(s: *mut c_char, t: *const c_char) -> *mut c_char {
    if t.is_null() {
        return sdscpylen(s, ptr::null(), 0);
    }
    sdscpylen(s, t, libc::strlen(t))
}

#[no_mangle]
pub unsafe extern "C" fn sdsfreesplitres(tokens: *mut *mut c_char, count: c_int) {
    if tokens.is_null() {
        return;
    }

    let n = if count < 0 { 0 } else { count as usize };
    for i in 0..n {
        let token = *tokens.add(i);
        if !token.is_null() {
            sdsfree(token);
        }
    }
    alloc_free(tokens as *mut c_void);
}

#[no_mangle]
pub unsafe extern "C" fn redisFormatCommandArgv(
    target: *mut *mut c_char,
    argc: c_int,
    argv: *const *const c_char,
    argvlen: *const size_t,
) -> c_longlong {
    if target.is_null() || argc < 0 {
        return -1;
    }
    if argc > 0 && argv.is_null() {
        return -1;
    }

    let Some(resp) = build_resp_command(argc, argv, argvlen) else {
        return -1;
    };

    let mem = alloc_malloc(resp.len() + 1) as *mut c_char;
    if mem.is_null() {
        return -1;
    }

    ptr::copy_nonoverlapping(resp.as_ptr() as *const c_char, mem, resp.len());
    *mem.add(resp.len()) = 0;
    *target = mem;

    resp.len() as c_longlong
}

#[no_mangle]
pub unsafe extern "C" fn redisvFormatCommand(
    target: *mut *mut c_char,
    _format: *const c_char,
    _ap: *mut c_void,
) -> c_int {
    if !target.is_null() {
        *target = ptr::null_mut();
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn redisFormatCommand(
    target: *mut *mut c_char,
    _format: *const c_char,
) -> c_int {
    if !target.is_null() {
        *target = ptr::null_mut();
    }
    -1
}

#[no_mangle]
pub unsafe extern "C" fn redisFreeCommand(cmd: *mut c_char) {
    if cmd.is_null() {
        return;
    }
    alloc_free(cmd as *mut c_void)
}

#[no_mangle]
pub unsafe extern "C" fn redisFormatSdsCommandArgv(
    target: *mut *mut c_char,
    argc: c_int,
    argv: *const *const c_char,
    argvlen: *const size_t,
) -> c_longlong {
    if target.is_null() {
        return -1;
    }

    let mut raw: *mut c_char = ptr::null_mut();
    let len = redisFormatCommandArgv(&mut raw as *mut *mut c_char, argc, argv, argvlen);
    if len < 0 || raw.is_null() {
        return -1;
    }

    let s = sdsnewlen(raw as *const c_void, len as size_t);
    redisFreeCommand(raw);
    if s.is_null() {
        return -1;
    }

    *target = s;
    len
}

#[no_mangle]
pub unsafe extern "C" fn redisFreeSdsCommand(cmd: *mut c_char) {
    sdsfree(cmd)
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
pub extern "C" fn redisReaderCreateWithFunctions(fn_: *mut redisReplyObjectFunctions) -> *mut redisReader {
    let reader = Box::new(redisReader {
        err: REDIS_OK,
        errstr: [0; 128],
        buf: ptr::null_mut(),
        pos: 0,
        len: 0,
        maxbuf: REDIS_READER_MAX_BUF,
        maxelements: REDIS_READER_MAX_ARRAY_ELEMENTS,
        task: ptr::null_mut(),
        tasks: 0,
        ridx: -1,
        reply: ptr::null_mut(),
        fn_,
        privdata: ptr::null_mut(),
    });

    let ptr = Box::into_raw(reader);
    if let Ok(mut states) = reader_states().lock() {
        states.insert(ptr as usize, ReaderState::default());
    }
    ptr
}

#[no_mangle]
pub extern "C" fn redisReaderCreate() -> *mut redisReader {
    redisReaderCreateWithFunctions(ptr::null_mut())
}

#[no_mangle]
pub extern "C" fn redisReaderFree(r: *mut redisReader) {
    if r.is_null() {
        return;
    }

    if let Ok(mut states) = reader_states().lock() {
        states.remove(&(r as usize));
    }

    unsafe {
        // When custom object functions are installed (e.g., hiredis-py), reply
        // ownership belongs to the caller/runtime, not this reader.
        if (*r).fn_.is_null() && !(*r).reply.is_null() {
            freeReplyObject((*r).reply);
            (*r).reply = ptr::null_mut();
        }
        drop(Box::from_raw(r));
    }
}

#[no_mangle]
pub extern "C" fn redisReaderFeed(r: *mut redisReader, buf: *const c_char, len: size_t) -> c_int {
    if r.is_null() {
        return REDIS_ERR;
    }

    unsafe {
        clear_reader_error(r);
    }

    let mut states = match reader_states().lock() {
        Ok(guard) => guard,
        Err(_) => {
            unsafe {
                set_reader_error(r, REDIS_ERR_OTHER, "reader state lock poisoned");
            }
            return REDIS_ERR;
        }
    };

    let Some(state) = states.get_mut(&(r as usize)) else {
        unsafe {
            set_reader_error(r, REDIS_ERR_OTHER, "reader state missing");
        }
        return REDIS_ERR;
    };

    if len > 0 {
        if buf.is_null() {
            unsafe {
                set_reader_error(r, REDIS_ERR_PROTOCOL, "null buffer with positive length");
            }
            return REDIS_ERR;
        }

        unsafe {
            let incoming = slice::from_raw_parts(buf as *const u8, len);
            state.buffer.extend_from_slice(incoming);
        }
    }

    unsafe {
        update_reader_metrics(r, state);
    }

    REDIS_OK
}

#[no_mangle]
pub extern "C" fn redisReaderGetReply(r: *mut redisReader, reply: *mut *mut c_void) -> c_int {
    if r.is_null() {
        return REDIS_ERR;
    }

    unsafe {
        clear_reader_error(r);
    }

    let mut states = match reader_states().lock() {
        Ok(guard) => guard,
        Err(_) => {
            unsafe {
                set_reader_error(r, REDIS_ERR_OTHER, "reader state lock poisoned");
            }
            return REDIS_ERR;
        }
    };

    let Some(state) = states.get_mut(&(r as usize)) else {
        unsafe {
            set_reader_error(r, REDIS_ERR_OTHER, "reader state missing");
        }
        return REDIS_ERR;
    };

    let parse_result = unsafe { parse_and_build(r, &state.buffer, 0, ptr::null_mut(), 0) };

    match parse_result {
        Ok((obj, consumed)) => {
            if consumed > 0 {
                state.buffer.drain(0..consumed);
            }

            unsafe {
                update_reader_metrics(r, state);
                (*r).reply = obj;
                if !reply.is_null() {
                    *reply = obj;
                }
            }
            REDIS_OK
        }
        Err(ParseError::Incomplete) => {
            unsafe {
                update_reader_metrics(r, state);
                if !reply.is_null() {
                    *reply = ptr::null_mut();
                }
                (*r).reply = ptr::null_mut();
            }
            REDIS_OK
        }
        Err(ParseError::Protocol(msg)) => {
            unsafe {
                set_reader_error(r, REDIS_ERR_PROTOCOL, &msg);
                if !reply.is_null() {
                    *reply = ptr::null_mut();
                }
                (*r).reply = ptr::null_mut();
            }
            REDIS_ERR
        }
    }
}

#[no_mangle]
pub extern "C" fn freeReplyObject(reply: *mut c_void) {
    if reply.is_null() {
        return;
    }

    unsafe {
        free_reply_recursive(reply as *mut redisReply);
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
