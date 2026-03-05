use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let upstream = manifest_dir
        .join("../../subprojects/redispy-hiredis-compat/vendor/hiredis-py/vendor/hiredis")
        .canonicalize()
        .expect("upstream hiredis source tree not found; run submodule bootstrap");

    let sources = [
        "alloc.c",
        "dict.c",
        "hiredis.c",
        "net.c",
        "read.c",
        "sds.c",
        "sockcompat.c",
        "async.c",
    ];

    for src in &sources {
        println!("cargo:rerun-if-changed={}", upstream.join(src).display());
    }
    for hdr in [
        "alloc.h",
        "dict.h",
        "fmacros.h",
        "hiredis.h",
        "net.h",
        "read.h",
        "sds.h",
        "sdsalloc.h",
        "sockcompat.h",
        "async.h",
        "async_private.h",
        "win32.h",
    ] {
        println!("cargo:rerun-if-changed={}", upstream.join(hdr).display());
    }

    let mut build = cc::Build::new();
    build
        .include(&upstream)
        .define("_DEFAULT_SOURCE", None)
        .warnings(false)
        .files(sources.iter().map(|s| upstream.join(s)));

    if cfg!(target_os = "linux") {
        build.define("_GNU_SOURCE", None);
    }

    build.compile("hiredis_upstream");
}
