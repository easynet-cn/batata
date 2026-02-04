fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Note: Proto compilation is disabled for now.
    // We use native Rust types instead of generated proto types.
    // Proto files are kept for reference and future gRPC service implementation.
    //
    // To enable proto compilation, uncomment the following:
    // tonic_prost_build::configure()
    //     .build_server(true)
    //     .build_client(true)
    //     .out_dir("src/xds/")
    //     .compile_protos(
    //         &[
    //             "proto/xds.proto",
    //             "proto/eds.proto",
    //             "proto/cds.proto",
    //         ],
    //         &["proto/"],
    //     )?;

    // Tell Cargo to rerun if proto files change
    println!("cargo:rerun-if-changed=proto/");

    Ok(())
}
