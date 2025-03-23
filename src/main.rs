use std::{
    io::{self},
    sync::mpsc,
    thread::{self},
    time::Duration,
};

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Stylize},
    symbols::border,
    text::Span,
    widgets::{Block, Gauge, List, ListItem, Widget},
    DefaultTerminal, Frame,
};
use serde::{Deserialize, Serialize};
use tungstenite::{connect, Message};

fn main() -> io::Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App {
        exit: false,
        progress_bar_color: Color::LightRed,
        progress: 0_f64,
        messages: vec![],
        tasks: vec![],
    };
    let (tx, rx) = mpsc::channel::<Event>();

    let tx_input_clone = tx.clone();
    thread::spawn(move || {
        handle_input_events(tx_input_clone);
    });

    let tx_websocket_clone = tx.clone();
    thread::spawn(move || {
        handle_websocket_events(tx_websocket_clone);
    });

    let app_result = app.run(&mut terminal, rx);
    ratatui::restore();
    app_result
}

pub enum Event {
    Input(crossterm::event::KeyEvent),
    Progress(f64),
    Message(String),
    TaskUpdate(TaskEvent),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "event_type")]
pub enum TaskEvent {
    #[serde(rename = "TaskInit")]
    TaskInit {
        task_id: String,
        name: String,
        created_at: f64,
        priority: u8,
    },
    #[serde(rename = "TaskUpdate")]
    TaskUpdate {
        task_id: String,
        status: String,
        progress: f64,
        timestamp: f64,
    },
    #[serde(rename = "TaskMetrics")]
    TaskMetrics {
        task_id: String,
        cpu_usage: f64,
        memory_usage: f64,
        disk_io: f64,
        timestamp: f64,
    },
    #[serde(rename = "TaskLogs")]
    TaskLogs {
        task_id: String,
        log_level: String,
        message: String,
        timestamp: f64,
    },
    #[serde(rename = "TaskTerminate")]
    TaskTerminate {
        task_id: String,
        exit_code: i32,
        reason: String,
        timestamp: f64,
    },
}

#[derive(Debug, Clone)]
pub struct Task {
    id: String,
    name: String,
    status: String,
    progress: f64,
    last_update: f64,
}

fn handle_input_events(tx: mpsc::Sender<Event>) {
    loop {
        match crossterm::event::read().unwrap() {
            crossterm::event::Event::Key(key_event) => tx.send(Event::Input(key_event)).unwrap(),
            _ => {}
        }
    }
}

fn handle_websocket_events(tx: mpsc::Sender<Event>) {
    // Fix: Use a string URL instead of url::Url
    let url = "ws://localhost:8765";

    match connect(url) {
        Ok((mut socket, _)) => {
            println!("WebSocket connection established");

            loop {
                // Fix: Use read() instead of deprecated read_message()
                match socket.read() {
                    Ok(Message::Text(text)) => {
                        match serde_json::from_str::<TaskEvent>(&text) {
                            Ok(event) => {
                                tx.send(Event::TaskUpdate(event.clone())).unwrap();

                                // Also send a readable message for the log
                                let msg = match event {
                                    TaskEvent::TaskInit { task_id, name, .. } => {
                                        format!("Task initialized: {} ({})", name, task_id)
                                    }
                                    TaskEvent::TaskUpdate {
                                        task_id,
                                        status,
                                        progress,
                                        ..
                                    } => {
                                        format!(
                                            "Task {}: {} - {:.0}%",
                                            task_id,
                                            status,
                                            progress * 100.0
                                        )
                                    }
                                    TaskEvent::TaskLogs {
                                        task_id,
                                        log_level,
                                        message,
                                        ..
                                    } => {
                                        format!("Task {} [{}]: {}", task_id, log_level, message)
                                    }
                                    TaskEvent::TaskMetrics {
                                        task_id,
                                        cpu_usage,
                                        memory_usage,
                                        ..
                                    } => {
                                        format!(
                                            "Task {} metrics: CPU {:.1}%, Mem {:.1}MB",
                                            task_id, cpu_usage, memory_usage
                                        )
                                    }
                                    TaskEvent::TaskTerminate {
                                        task_id,
                                        exit_code,
                                        reason,
                                        ..
                                    } => {
                                        format!(
                                            "Task {} terminated: {} (code: {})",
                                            task_id, reason, exit_code
                                        )
                                    }
                                };
                                tx.send(Event::Message(msg)).unwrap();
                            }
                            Err(e) => {
                                tx.send(Event::Message(format!("Error parsing message: {}", e)))
                                    .unwrap();
                            }
                        }
                    }
                    Ok(_) => {} // Ignore non-text messages
                    Err(e) => {
                        tx.send(Event::Message(format!("WebSocket error: {}", e)))
                            .unwrap();
                        thread::sleep(Duration::from_secs(5));
                        break;
                    }
                }
            }
        }
        Err(e) => {
            tx.send(Event::Message(format!("Failed to connect: {}", e)))
                .unwrap();
            thread::sleep(Duration::from_secs(5));
        }
    }
}

pub struct App {
    exit: bool,
    progress_bar_color: Color,
    progress: f64,
    messages: Vec<String>,
    tasks: Vec<Task>,
}

impl App {
    pub fn run(
        &mut self,
        terminal: &mut DefaultTerminal,
        rx: mpsc::Receiver<Event>,
    ) -> io::Result<()> {
        while !self.exit {
            match rx.recv().unwrap() {
                Event::Input(key_event) => self.handle_key_event(key_event)?,
                Event::Progress(progress) => self.progress = progress,
                Event::Message(msg) => {
                    self.messages.push(msg);
                    if self.messages.len() > 10 {
                        self.messages.remove(0);
                    }
                }
                Event::TaskUpdate(event) => self.handle_task_event(event),
            }
            terminal.draw(|frame| self.draw(frame))?;
        }
        Ok(())
    }

    fn handle_task_event(&mut self, event: TaskEvent) {
        match event {
            TaskEvent::TaskInit { task_id, name, .. } => {
                self.tasks.push(Task {
                    id: task_id,
                    name,
                    status: "initialized".to_string(),
                    progress: 0.0,
                    last_update: Utc::now().timestamp() as f64,
                });
            }
            TaskEvent::TaskUpdate {
                task_id,
                status,
                progress,
                timestamp,
            } => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = status;
                    task.progress = progress;
                    task.last_update = timestamp;
                }
            }
            TaskEvent::TaskTerminate { task_id, .. } => {
                if let Some(task) = self.tasks.iter_mut().find(|t| t.id == task_id) {
                    task.status = "terminated".to_string();
                }
            }
            _ => {} // Handle other events if needed
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    pub fn handle_key_event(&mut self, key_event: KeyEvent) -> io::Result<()> {
        if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Char('q') {
            self.exit = true;
        }
        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        let layout = Layout::new(
            Direction::Vertical,
            [Constraint::Length(5), Constraint::Min(5)],
        )
        .split(area);

        // Create task status blocks
        let block = Block::bordered()
            .title("Background Processes")
            .title_top("Task Monitor".bold().blue())
            .title_bottom("Press Q to quit".bold().blue())
            .border_set(border::THICK);

        // Use the progress from the most recent task or default
        let current_progress = self
            .tasks
            .iter()
            .filter(|t| t.status == "running")
            .map(|t| t.progress)
            .next()
            .unwrap_or(self.progress);

        let progress_bar = Gauge::default()
            .gauge_style(Style::default().fg(self.progress_bar_color))
            .block(block)
            .label(format!("Progress: {:.2}%", current_progress * 100.0))
            .ratio(current_progress);

        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .map(|msg| ListItem::new(Span::raw(msg.clone())))
            .collect();

        let messages_list = List::new(messages)
            .block(Block::bordered().title("Task Messages"))
            .style(Style::default().fg(Color::White));

        // Fix: Use proper Widget rendering
        progress_bar.render(layout[0], buf);
        messages_list.render(layout[1], buf);
    }
}
