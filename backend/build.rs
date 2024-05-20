fn main() {
    prost_build::compile_protos(&["src/protos/gtfs-realtime-NYCT.proto"], &["src/protos"]).unwrap();
}
