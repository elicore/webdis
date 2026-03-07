use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=REDIS_WEB_HIREDIS_SRC_DIR");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let upstream = resolve_hiredis_source_dir(&manifest_dir);

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

fn resolve_hiredis_source_dir(manifest_dir: &PathBuf) -> PathBuf {
    let mut candidates = Vec::new();

    if let Ok(override_dir) = env::var("REDIS_WEB_HIREDIS_SRC_DIR") {
        let override_path = PathBuf::from(override_dir);
        candidates.push(("REDIS_WEB_HIREDIS_SRC_DIR", override_path));
    }

    candidates.push((
        "submodule",
        manifest_dir
            .join("../../subprojects/redispy-hiredis-compat/vendor/hiredis-py/vendor/hiredis"),
    ));
    candidates.push(("vendored", manifest_dir.join("vendor/hiredis")));

    for (source, candidate) in &candidates {
        if candidate.join("hiredis.c").is_file() {
            let resolved = candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.to_path_buf());
            println!(
                "cargo:warning=redis-web-hiredis-compat using hiredis source from {}: {}",
                source,
                resolved.display()
            );
            return resolved;
        }
    }

    let searched = candidates
        .iter()
        .map(|(source, path)| format!("  - {}: {}", source, path.display()))
        .collect::<Vec<_>>()
        .join("\n");

    panic!(
        "unable to locate hiredis C sources.\nsearched:\n{}\n\n\
run one of:\n  - make compat_redispy_bootstrap\n  - git submodule update --init --recursive\n\n\
or set REDIS_WEB_HIREDIS_SRC_DIR=/absolute/path/to/hiredis",
        searched
    );
}
