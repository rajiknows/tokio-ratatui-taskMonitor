**Task Monitor**

Task Monitor is a Rust-based system for simulating and monitoring task execution in real-time. It consists of a gRPC server that generates task events and a terminal-based client (TUI) that displays these events. The project leverages Rust's asynchronous capabilities with Tokio, gRPC for communication, and Ratatui for the terminal interface. It simulates a genomic analysis workflow with tasks that can initialize, update, log messages, report metrics, and terminate.
***Features***

    gRPC Server: Streams simulated task events including initialization, updates, logs, metrics, and termination.
    TUI Client: Displays task statuses, progress, metrics, and logs in a responsive terminal interface.
    Task Simulation: Randomly generates task events with realistic log messages (e.g., "Aligning sequences...") and performance metrics (CPU, memory, disk I/O).
    Real-Time Updates: Updates the UI every 200ms and streams events with random delays for a lifelike experience.
    Interactive Controls: Scroll through logs with arrow keys and quit the client with 'q'.

***Prerequisites***

    Rust (latest stable version recommended)
    Cargo (Rust's package manager)
    protoc (Protocol Buffers compiler) for generating gRPC code
    A terminal emulator supporting raw mode (e.g., most Unix terminals or Windows Terminal)

***Setup***

    Clone the Repository
    bash

git clone https://github.com/yourusername/task-monitor.git
cd task-monitor
Install Dependencies Ensure you have Rust and Cargo installed. Then, install the required protoc compiler:

    On Ubuntu: sudo apt install protobuf-compiler
    On macOS: brew install protobuf
    On Windows: Download from protobuf releases and add to PATH.

Generate gRPC Code The project assumes a task_monitor.proto file defining the gRPC service. If not already generated, you'll need to compile it:
bash
cargo install tonic-build
Then, ensure the proto file is in the project root and run your build script or command to generate Rust code (e.g., via a build.rs file).
Build the Project
bash

    cargo build --release

***Usage***
Running the Server

The server simulates task events and streams them via gRPC on localhost:50051.
bash
cargo run --bin server

    Replace server with the actual binary name if you've split the server and client into separate binaries (e.g., via [[bin]] sections in Cargo.toml).

Running the Client

The client connects to the server and displays task information in the terminal.
bash
cargo run --bin client

    Similarly, replace client with the actual binary name if customized.

Interacting with the Client

    Quit: Press q to exit the client.
    Scroll Logs: Use the Up and Down arrow keys to navigate through the message log.

***Project Structure***

    src/server.rs: Contains the gRPC server implementation (TaskMonitorService) that simulates task events.
    src/client.rs: Implements the TUI client using Ratatui, displaying task data streamed from the server.
    proto/task_monitor.proto: Defines the gRPC service and message types (e.g., TaskEvent, TaskStatus).

Server Details

    Simulates 5 tasks by default if no task IDs are provided in the WatchRequest.
    Generates events randomly: updates (25%), metrics (25%), logs (25%), and terminations (25%).
    Resets tasks after 100 events for continuous simulation.

Client Details

    Displays a header, task list, and message log in a vertical layout.
    Updates every 200ms via a tick event.
    Supports up to 50 log messages and 10 logs per task, with older entries removed.

Example Output

![image](https://github.com/user-attachments/assets/00557e8e-97f9-4228-8328-30e079df0dbf)

