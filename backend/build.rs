fn main() {
    prost_build::Config::new()
        .compile_protos(&["src/protos/gtfs-realtime.proto"], &["src/protos"])
        .unwrap();
}
