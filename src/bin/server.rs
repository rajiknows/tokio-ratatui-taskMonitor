use futures::stream::Stream;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::pin::Pin;
use std::time::SystemTime;
use tokio::sync::mpsc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

use task_monitor::taskmonitor::{
    task_monitor_service_server::{
        TaskMonitorService as TaskMonitorServiceTrait, TaskMonitorServiceServer,
    },
    LogLevel, TaskEvent, TaskInitEvent, TaskLogEvent, TaskMetricsEvent, TaskStatus,
    TaskTerminateEvent, TaskUpdateEvent, WatchRequest,
};

/// A gRPC service implementation for monitoring tasks.
///
/// This struct provides the server-side logic for streaming task events to clients,
/// simulating task lifecycle events such as initialization, updates, logs, metrics,
/// and termination.
#[derive(Default)]
pub struct TaskMonitorService;

/// Predefined log messages for task simulation.
///
/// These messages are used to generate realistic task logs with optional placeholders
/// for dynamic content (e.g., chromosome numbers or subtask IDs).
const LOG_MESSAGES: [&str; 10] = [
    "Loading reference genome...",
    "Aligning sequences...",
    "Processing chromosome {}...",
    "Analyzing variants...",
    "Calculating statistics...",
    "Optimizing memory usage...",
    "Generating report...",
    "Validating results...",
    "Waiting for resources...",
    "Executing subtask {}...",
];

#[tonic::async_trait]
impl TaskMonitorServiceTrait for TaskMonitorService {
    /// The type of stream returned by the `watch_task_events` method.
    ///
    /// This is a pinned, boxed stream of `Result<TaskEvent, Status>` that is safe to send
    /// across threads.
    type WatchTaskEventsStream = Pin<Box<dyn Stream<Item = Result<TaskEvent, Status>> + Send>>;

    /// Streams task events to the client based on the provided `WatchRequest`.
    ///
    /// If no task IDs are specified in the request, it generates 5 random tasks. The method
    /// simulates task lifecycle events (init, update, log, metrics, terminate) using random
    /// data and sends them through a gRPC stream. Tasks can terminate and optionally reset
    /// after a certain number of events.
    async fn watch_task_events(
        &self,
        request: Request<WatchRequest>,
    ) -> Result<Response<Self::WatchTaskEventsStream>, Status> {
        // Create a channel for sending task events
        let (tx, rx) = mpsc::channel(128);

        let req = request.into_inner();
        let task_ids: Vec<String> = if req.task_ids.is_empty() {
            // Generate 5 random task IDs if none provided
            (0..5)
                .map(|_| Uuid::new_v4().to_string()[..8].to_string())
                .collect()
        } else {
            req.task_ids.clone()
        };

        // Spawn an async task to simulate and stream task events
        tokio::spawn(async move {
            let mut rng = StdRng::from_entropy(); // Thread-safe RNG
            let mut active_tasks: std::collections::HashMap<_, _> =
                task_ids.iter().map(|id| (id.clone(), true)).collect();
            let mut counter = 0;

            // Initialize all tasks with an `Init` event
            for (index, task_id) in task_ids.iter().enumerate() {
                let init_event = TaskEvent {
                    event: Some(task_monitor::taskmonitor::task_event::Event::Init(
                        TaskInitEvent {
                            task_id: task_id.clone(),
                            name: format!("Genomic Analysis Task {}", index + 1),
                            created_at: SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap()
                                .as_secs_f64(),
                            priority: rng.gen_range(1..=5),
                        },
                    )),
                };

                if tx.send(Ok(init_event)).await.is_err() {
                    break; // Client disconnected or channel closed
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            }

            // Main event simulation loop
            while !active_tasks.is_empty() {
                let active_task_ids: Vec<_> = active_tasks
                    .iter()
                    .filter(|(_, &active)| active)
                    .map(|(id, _)| id.clone())
                    .collect();

                if active_task_ids.is_empty() {
                    break;
                }

                let task_id = active_task_ids[rng.gen_range(0..active_task_ids.len())].clone();

                let event = match rng.gen_range(0..4) {
                    0 => {
                        // Simulate a task update event
                        let status = match rng.gen_range(0..4) {
                            0 => TaskStatus::Waiting,
                            1 => TaskStatus::Running,
                            2 => TaskStatus::Completed,
                            _ => TaskStatus::Failed,
                        };

                        let update_event = TaskEvent {
                            event: Some(task_monitor::taskmonitor::task_event::Event::Update(
                                TaskUpdateEvent {
                                    task_id: task_id.clone(),
                                    status: status as i32,
                                    progress: rng.gen(), // Random progress between 0.0 and 1.0
                                    timestamp: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs_f64(),
                                },
                            )),
                        };

                        // Randomly terminate completed or failed tasks with 20% probability
                        if matches!(status, TaskStatus::Completed | TaskStatus::Failed)
                            && rng.gen_bool(0.2)
                        {
                            active_tasks.insert(task_id.clone(), false);
                        }

                        update_event
                    }
                    1 => {
                        // Simulate a task metrics event
                        TaskEvent {
                            event: Some(task_monitor::taskmonitor::task_event::Event::Metrics(
                                TaskMetricsEvent {
                                    task_id: task_id.clone(),
                                    cpu_usage: rng.gen_range(0.1..100.0),
                                    memory_usage: rng.gen_range(50.0..8192.0),
                                    disk_io: rng.gen_range(0.1..50.0),
                                    timestamp: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs_f64(),
                                },
                            )),
                        }
                    }
                    2 => {
                        // Simulate a task log event
                        let mut msg =
                            LOG_MESSAGES[rng.gen_range(0..LOG_MESSAGES.len())].to_string();
                        if msg.contains("{}") {
                            msg = msg.replace("{}", &rng.gen_range(1..=22).to_string());
                        }

                        TaskEvent {
                            event: Some(task_monitor::taskmonitor::task_event::Event::Log(
                                TaskLogEvent {
                                    task_id: task_id.clone(),
                                    log_level: match rng.gen_range(0..4) {
                                        0 => LogLevel::Info,
                                        1 => LogLevel::Debug,
                                        2 => LogLevel::Warning,
                                        _ => LogLevel::Error,
                                    } as i32,
                                    message: msg,
                                    timestamp: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs_f64(),
                                },
                            )),
                        }
                    }
                    _ => {
                        // Simulate a task termination event
                        let terminate_event = TaskEvent {
                            event: Some(task_monitor::taskmonitor::task_event::Event::Terminate(
                                TaskTerminateEvent {
                                    task_id: task_id.clone(),
                                    exit_code: if rng.gen_bool(0.8) { 0 } else { 1 }, // 80% success rate
                                    reason: if rng.gen_bool(0.8) {
                                        "Completed successfully".to_string()
                                    } else {
                                        "Failed with error".to_string()
                                    },
                                    timestamp: SystemTime::now()
                                        .duration_since(SystemTime::UNIX_EPOCH)
                                        .unwrap()
                                        .as_secs_f64(),
                                },
                            )),
                        };

                        active_tasks.insert(task_id.clone(), false);
                        terminate_event
                    }
                };

                if tx.send(Ok(event)).await.is_err() {
                    break; // Client disconnected or channel closed
                }

                // Random delay between events for realistic pacing
                tokio::time::sleep(tokio::time::Duration::from_millis(rng.gen_range(300..1000)))
                    .await;

                counter += 1;

                // Reset all tasks to active after 100 events for continuous simulation
                if counter > 100 {
                    active_tasks = task_ids.iter().map(|id| (id.clone(), true)).collect();
                    counter = 0;
                }
            }
        });

        // Return the stream as a gRPC response
        Ok(Response::new(Box::pin(
            tokio_stream::wrappers::ReceiverStream::new(rx),
        )))
    }
}

/// Starts the gRPC server hosting the `TaskMonitorService`.
///
/// Binds the server to `[::1]:50051` and serves task monitoring events until interrupted.
pub async fn start_server() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let task_monitor = TaskMonitorService::default();

    println!("Starting server on {}", addr);

    tonic::transport::Server::builder()
        .add_service(TaskMonitorServiceServer::new(task_monitor))
        .serve(addr)
        .await?;

    Ok(())
}

/// Application entry point for running the gRPC server.
///
/// Uses Tokio's async runtime to initialize and start the server.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    start_server().await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests that the server can be initialized without errors.
    ///
    /// This is a basic smoke test to ensure the server starts up correctly.
    #[tokio::test]
    async fn test_server_initialization() {
        assert!(start_server().await.is_ok());
    }
}
