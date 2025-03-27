use std::{
    collections::HashMap,
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{KeyCode, KeyEvent, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use tokio::{sync::mpsc, task};
use tonic::Request;

// Import generated gRPC code from the task_monitor proto definitions
use task_monitor::taskmonitor::{
    task_event::Event, task_monitor_service_client::TaskMonitorServiceClient, LogLevel, TaskStatus,
};

/// Represents a task with detailed tracking information.
///
/// This struct holds all relevant data about a task, including its status, progress,
/// logs, and optional performance metrics.
#[derive(Debug, Clone)]
pub struct Task {
    id: String,
    name: String,
    status: TaskStatus,
    progress: f64,
    logs: Vec<TaskLog>,
    metrics: Option<TaskMetrics>,
    last_update: Instant,
}

/// Represents a single log entry for a task.
///
/// Logs include a timestamp, log level, and message to track task activity over time.
#[derive(Debug, Clone)]
pub struct TaskLog {
    timestamp: Instant,
    level: LogLevel,
    message: String,
}

/// Holds performance metrics for a task.
///
/// Metrics include CPU usage, memory usage, and disk I/O, providing insight into
/// resource consumption.
#[derive(Debug, Clone)]
pub struct TaskMetrics {
    cpu_usage: f64,
    memory_usage: f64,
    disk_io: f64,
}

/// Defines events that the application can handle.
///
/// This enum encapsulates user input, task updates from the gRPC stream, and periodic ticks
/// for UI refresh.
pub enum AppEvent {
    Input(KeyEvent),
    TaskEvent(task_monitor::taskmonitor::TaskEvent),
    Tick,
}

/// Main application state and logic.
///
/// Manages a collection of tasks, log messages, and UI state such as scrolling and exit conditions.
pub struct App {
    exit: bool,
    tasks: HashMap<String, Task>,
    messages: Vec<String>,
    scroll_offset: usize,
}

impl Default for App {
    /// Creates a new `App` instance with default values.
    fn default() -> Self {
        Self {
            exit: false,
            tasks: HashMap::new(),
            messages: Vec::new(),
            scroll_offset: 0,
        }
    }
}

impl App {
    /// Runs the application, setting up the terminal and event loop.
    ///
    /// This method initializes the terminal in raw mode, sets up event channels for input,
    /// periodic ticks, and gRPC task events, and runs the main event loop until exit.
    pub async fn run(&mut self) -> io::Result<()> {
        // Enable raw mode for direct terminal input handling
        enable_raw_mode()?;

        // Initialize the terminal with the Crossterm backend
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

        // Create a channel for sending and receiving application events
        let (tx, mut rx) = mpsc::channel(100);
        let tx_input = tx.clone();
        let tx_task = tx.clone();

        // Spawn a task to handle user input asynchronously
        task::spawn(async move {
            loop {
                if let Ok(crossterm::event::Event::Key(key_event)) = crossterm::event::read() {
                    if tx_input.send(AppEvent::Input(key_event)).await.is_err() {
                        break; // Channel closed, exit the loop
                    }
                }
            }
        });

        // Spawn a task for periodic UI updates (tick every 200ms)
        let tx_tick = tx.clone();
        task::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(200)).await;
                if tx_tick.send(AppEvent::Tick).await.is_err() {
                    break; // Channel closed, exit the loop
                }
            }
        });

        // Spawn a task to handle gRPC task event streaming
        task::spawn(async move {
            let mut client = TaskMonitorServiceClient::connect("http://[::1]:50051")
                .await
                .unwrap();

            let request =
                Request::new(task_monitor::taskmonitor::WatchRequest { task_ids: vec![] });

            let mut stream = client
                .watch_task_events(request)
                .await
                .unwrap()
                .into_inner();

            while let Ok(Some(event)) = stream.message().await {
                if tx_task.send(AppEvent::TaskEvent(event)).await.is_err() {
                    break; // Channel closed, exit the loop
                }
            }
        });

        // Main event loop: process events and update UI until exit
        while !self.exit {
            if let Some(event) = rx.recv().await {
                match event {
                    AppEvent::Input(key_event) => self.handle_key_event(key_event)?,
                    AppEvent::TaskEvent(task_event) => self.handle_task_event(task_event),
                    AppEvent::Tick => {}
                }

                terminal.draw(|frame| self.ui(frame))?;
            }
        }

        // Restore terminal to normal mode before exiting
        disable_raw_mode()?;
        Ok(())
    }

    /// Handles keyboard input events.
    ///
    /// Supports quitting the app with 'q', scrolling up with the Up arrow, and scrolling
    /// down with the Down arrow.
    fn handle_key_event(&mut self, key_event: KeyEvent) -> io::Result<()> {
        if key_event.kind == KeyEventKind::Press {
            match key_event.code {
                KeyCode::Char('q') => self.exit = true,
                KeyCode::Up => self.scroll_offset = self.scroll_offset.saturating_sub(1),
                KeyCode::Down => self.scroll_offset += 1,
                _ => {}
            }
        }
        Ok(())
    }

    /// Processes task events received from the gRPC stream.
    ///
    /// Updates task state based on initialization, updates, logs, metrics, or termination events,
    /// and logs relevant messages.
    fn handle_task_event(&mut self, event: task_monitor::taskmonitor::TaskEvent) {
        let mut log_message = None;

        match event.event {
            Some(Event::Init(init)) => {
                let task = Task {
                    id: init.task_id.clone(),
                    name: init.name.clone(),
                    status: TaskStatus::Waiting,
                    progress: 0.0,
                    logs: Vec::new(),
                    metrics: None,
                    last_update: Instant::now(),
                };
                self.tasks.insert(init.task_id.clone(), task);
                log_message = Some(format!("Task initialized: {}", init.name));
            }
            Some(Event::Update(update)) => {
                if let Some(task) = self.tasks.get_mut(&update.task_id) {
                    task.status = update.status();
                    task.progress = update.progress;
                    task.last_update = Instant::now();
                    log_message = Some(format!(
                        "Task {} updated: {:?} - {:.1}%",
                        update.task_id,
                        task.status,
                        task.progress * 100.0
                    ));
                }
            }
            Some(Event::Log(log)) => {
                if let Some(task) = self.tasks.get_mut(&log.task_id) {
                    task.logs.push(TaskLog {
                        timestamp: Instant::now(),
                        level: log.log_level(),
                        message: log.message.clone(),
                    });
                    if task.logs.len() > 10 {
                        task.logs.remove(0); // Keep only the latest 10 logs
                    }
                    log_message = Some(format!(
                        "Task {} log: {:?} - {}",
                        log.task_id,
                        log.log_level(),
                        log.message
                    ));
                }
            }
            Some(Event::Metrics(metrics)) => {
                if let Some(task) = self.tasks.get_mut(&metrics.task_id) {
                    task.metrics = Some(TaskMetrics {
                        cpu_usage: metrics.cpu_usage,
                        memory_usage: metrics.memory_usage,
                        disk_io: metrics.disk_io,
                    });
                }
            }
            Some(Event::Terminate(term)) => {
                if let Some(task) = self.tasks.get_mut(&term.task_id) {
                    task.status = if term.exit_code == 0 {
                        TaskStatus::Completed
                    } else {
                        TaskStatus::Failed
                    };
                    log_message = Some(format!(
                        "Task {} terminated: {} (exit code: {})",
                        term.task_id, term.reason, term.exit_code
                    ));
                }
            }
            None => {}
        }

        if let Some(msg) = log_message {
            self.log_message(msg);
        }
    }

    /// Adds a message to the application's message log.
    ///
    /// Limits the message history to 50 entries, removing the oldest when the limit is exceeded.
    fn log_message(&mut self, message: String) {
        self.messages.push(message);
        if self.messages.len() > 50 {
            self.messages.remove(0);
        }
    }

    /// Renders the terminal UI using Ratatui.
    ///
    /// Displays a header, a task list with status and metrics, and a scrollable message log.
    fn ui(&self, frame: &mut Frame) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header section
                Constraint::Min(10),    // Tasks section (minimum height)
                Constraint::Length(10), // Messages section
            ])
            .split(frame.size());

        // Render header with title and instructions
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Task Monitor", Style::default().fg(Color::Blue)),
            Span::raw(" | "),
            Span::styled("Press 'q' to quit", Style::default().fg(Color::Gray)),
        ]))
        .block(Block::default().borders(Borders::BOTTOM));
        frame.render_widget(header, layout[0]);

        // Render tasks list
        let tasks_block = Block::default().title("Tasks").borders(Borders::ALL);
        let tasks_list: Vec<ListItem> = self
            .tasks
            .values()
            .map(|task| {
                let status_color = match task.status {
                    TaskStatus::Waiting => Color::Yellow,
                    TaskStatus::Running => Color::Cyan,
                    TaskStatus::Completed => Color::Green,
                    TaskStatus::Failed => Color::Red,
                };

                let mut lines = vec![
                    Line::from(vec![
                        Span::styled(format!("{} ", task.name), Style::default().fg(Color::White)),
                        Span::styled(format!("[{}]", task.id), Style::default().fg(Color::Gray)),
                    ]),
                    Line::from(vec![
                        Span::styled(
                            format!("Status: {:?}", task.status),
                            Style::default().fg(status_color),
                        ),
                        Span::raw(" | "),
                        Span::styled(
                            format!("Progress: {:.1}%", task.progress * 100.0),
                            Style::default().fg(Color::Blue),
                        ),
                    ]),
                ];

                if let Some(metrics) = &task.metrics {
                    lines.push(Line::from(vec![Span::styled(
                        format!(
                            "CPU: {:.1}% | Memory: {:.1}MB | Disk IO: {:.1}",
                            metrics.cpu_usage, metrics.memory_usage, metrics.disk_io
                        ),
                        Style::default().fg(Color::Gray),
                    )]));
                }

                ListItem::new(lines)
            })
            .collect();

        let tasks_list_widget = List::new(tasks_list).block(tasks_block);
        frame.render_widget(tasks_list_widget, layout[1]);

        // Render messages log with scrolling
        let messages_block = Block::default().title("Messages").borders(Borders::ALL);
        let messages_text: Vec<Line> = self
            .messages
            .iter()
            .rev()
            .skip(self.scroll_offset)
            .take(10) // Show up to 10 messages at a time
            .map(|msg| Line::from(Span::raw(msg.clone())))
            .collect();

        let messages_paragraph = Paragraph::new(messages_text)
            .block(messages_block)
            .wrap(Wrap { trim: false });
        frame.render_widget(messages_paragraph, layout[2]);
    }
}

/// Application entry point.
///
/// Initializes and runs the `App` instance using Tokio's async runtime.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::default();
    app.run().await?;
    Ok(())
}
