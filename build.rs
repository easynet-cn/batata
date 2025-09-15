fn main() {
    tonic_prost_build::configure()
        .out_dir("src/grpc/")
        .compile_protos(&["proto/nacos_grpc_service.proto"], &["proto"])
        .unwrap();
}
