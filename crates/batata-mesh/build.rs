fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Generate Envoy xDS v3 protocol messages and the ADS service from the
    // checked-in proto definitions. The generated types are included through
    // `tonic::include_proto!` in `src/xds/generated.rs` and are the wire format
    // used by the real gRPC server; the native Rust types in `xds/types.rs`
    // remain the internal resource model.
    println!("cargo:rerun-if-changed=proto/");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(
            &[
                "proto/xds.proto",
                "proto/eds.proto",
                "proto/cds.proto",
                "proto/lds.proto",
                "proto/rds.proto",
                "proto/hcm.proto",
                "proto/tcp_proxy.proto",
                "proto/rbac.proto",
                "proto/rbac_filter.proto",
            ],
            &["proto/"],
        )?;

    Ok(())
}
