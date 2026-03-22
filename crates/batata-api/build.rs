fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::configure()
        .file_descriptor_set_path(
            std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("raft_descriptor.bin"),
        )
        .compile_protos(&["../../proto/raft.proto"], &["../../proto/"])?;
    Ok(())
}
