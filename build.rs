fn main() {
    // Compile Nacos gRPC service proto
    tonic_prost_build::configure()
        .out_dir("src/api/grpc/")
        .compile_protos(&["proto/nacos_grpc_service.proto"], &["proto"])
        .unwrap();

    // Compile Raft consensus proto
    tonic_prost_build::configure()
        .out_dir("src/api/raft/")
        .compile_protos(&["proto/raft.proto"], &["proto"])
        .unwrap();
}
