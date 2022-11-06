fn main() {
        protoc_rust_grpc::Codegen::new()
        .out_dir("src")
        .inputs(&[
            "raft-service.proto",
        ])
        .rust_protobuf(true)
        .run()
        .expect("protoc-rust-grpc");
}
