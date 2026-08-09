fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Re-run build script when the proto file changes (it lives outside the
    // crate directory, so cargo's default file-tracking doesn't cover it).
    println!("cargo:rerun-if-changed=../../proto/raft.proto");

    tonic_prost_build::configure()
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("raft_descriptor.bin"),
        )
        .compile_protos(&["../../proto/raft.proto"], &["../../proto/"])?;
    Ok(())
}
