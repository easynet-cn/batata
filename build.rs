fn main() {
    tonic_prost_build::configure()
        .out_dir("src/api/grpc/")
        .compile_protos(&["proto/nacos_grpc_service.proto"], &["proto"])
        .unwrap();
}
