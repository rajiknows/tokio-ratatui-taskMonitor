use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Hardcoded output directory
    let out_dir = PathBuf::from("./generated");

    // Ensure the output directory exists
    std::fs::create_dir_all(&out_dir)?;

    // Location of protocol buffers
    let proto_file = "./proto/taskmonitor.proto";

    // Tell Cargo that if the protocol buffer changes, rerun this script
    println!("cargo:rerun-if-changed={}", proto_file);

    // Configure the protobuf compiler
    tonic_build::configure()
        .build_server(true)
        .file_descriptor_set_path(out_dir.join("task_monitor_descriptor.bin"))
        .compile(&[proto_file], &["./proto"])?;

    Ok(())
}
