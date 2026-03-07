fn main() {
    prost_build::compile_protos(&["src/protos/gtfs-realtime.proto"], &["src/protos"]).unwrap();
}
