use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=proto/redis_web/v1/gateway.proto");

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc should exist");
    unsafe {
        env::set_var("PROTOC", protoc);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR should be set"));
    let descriptor_path = out_dir.join("redis_web_descriptor.bin");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(descriptor_path)
        .compile_protos(&["proto/redis_web/v1/gateway.proto"], &["proto"])
        .expect("gRPC proto compilation should succeed");
}
