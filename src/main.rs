use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ez_sim_lib::{CommandError, Simulator, ProcessResult};
use ratatui::{prelude::*, widgets::*};
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

// Enum to represent the current mode of the application
#[derive(PartialEq)]
enum AppMode {
    Menu,
    Manual,
    SerialSelect,
    SerialListen,
    Exiting,
}

// Enum to represent which UI element is currently focused
#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Menu,
    Input,
    Logs,
    SerialPortList,
    BaudRateList,
}

// Messages for communication between the serial thread and the main TUI thread
enum SerialMessage {
    Log(String),
    Error(String),
}

// The main application state for the TUI
struct App<'a> {
    simulator: &'a mut Simulator,
    mode: AppMode,
    focus: Focus,
    logs: Vec<String>,
    input: String,
    menu_selection: usize,
    log_state: ListState,
    // --- Serial Mode State ---
    available_ports: Vec<String>,
    port_list_state: ListState,
    baud_rates: Vec<u32>,
    baud_rate_list_state: ListState,
    serial_rx: Option<Receiver<SerialMessage>>,
    serial_tx: Sender<SerialMessage>,
    serial_thread_handle: Option<thread::JoinHandle<()>>,
    serial_should_stop: Option<Arc<AtomicBool>>,
}

impl<'a> App<'a> {
    fn new(simulator: &'a mut Simulator) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut port_list_state = ListState::default();
        port_list_state.select(Some(0));
        let mut baud_rate_list_state = ListState::default();
        baud_rate_list_state.select(Some(0));

        Self {
            simulator,
            mode: AppMode::Menu,
            focus: Focus::Menu,
            logs: vec!["Welcome to the Endzone 250 Simulator!".to_string()],
            input: String::new(),
            menu_selection: 0,
            log_state: ListState::default(),
            available_ports: Vec::new(),
            port_list_state,
            // Invert the baud rates to show most common first
            baud_rates: vec![115200, 57600, 38400, 19200, 9600],
            baud_rate_list_state,
            serial_rx: Some(rx),
            serial_tx: tx,
            serial_thread_handle: None,
            serial_should_stop: None,
        }
    }

    // Helper to add a log entry
    fn log(&mut self, message: String) {
        self.logs.push(message);
        self.log_state.select(Some(0));
    }

    // Process a command and log the result
    fn process_command(&mut self, command: &str) {
        self.log(format!("> {}", command));
        match self.simulator.process_command(command.as_bytes()) {
            Ok(result) => {
                // First, log any debug messages from the simulator
                for debug_log in result.logs {
                    self.log(debug_log);
                }
                // Then, log the actual response if it exists
                if let Some(response) = result.response {
                    self.log(format!("< {}", response));
                }
            }
            Err(e) => {
                let error_msg = match e {
                    CommandError::InvalidFrame => "Invalid command frame. A valid command must be enclosed in '<...>'.".to_string(),
                    CommandError::TooShort => "Command content is too short.".to_string(),
                    CommandError::InvalidAddress(_) => "Invalid hexadecimal address in command.".to_string(),
                    CommandError::InvalidCommandId(_) => "Command ID is not a valid number.".to_string(),
                    CommandError::UnimplementedCommand(id) => format!("Command '{}' is not yet implemented.", id),
                    CommandError::InvalidParameter => "Command contains an invalid parameter.".to_string(),
                };
                self.log(format!("[ERROR] {}", error_msg));
            }
        }
    }

    // Scan for available serial ports
    fn scan_ports(&mut self) {
        self.available_ports = match serialport::available_ports() {
            Ok(ports) => ports.into_iter().map(|p| p.port_name).collect(),
            Err(_) => {
                self.log("[ERROR] Could not enumerate serial ports.".to_string());
                Vec::new()
            }
        };
        if self.available_ports.is_empty() {
            self.log("No serial ports found.".to_string());
        }
    }

    // Clean up the serial thread resources
    fn stop_serial_thread(&mut self) {
        if let Some(stop_flag) = self.serial_should_stop.take() {
            stop_flag.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.serial_thread_handle.take() {
            handle.join().expect("Failed to join serial thread");
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=========================");
    println!("  Endzone 250 Simulator  ");
    println!("=========================");

    print!("Enter RS-485 address (hex, default: 1F): ");
    io::stdout().flush().unwrap();

    let mut addr_input = String::new();
    io::stdin().read_line(&mut addr_input).unwrap();

    let simulator_address = match addr_input.trim() {
        "" => 0x1F,
        s => u8::from_str_radix(s, 16).unwrap_or_else(|_| {
            eprintln!("[WARNING] Invalid hex address '{}'. Using default 0x1F.", s);
            0x1F
        }),
    };

    let mut simulator = Simulator::new(simulator_address);
    println!("Simulator starting with Address: 0x{:02X}", simulator_address);
    println!("Launching TUI...");
    std::thread::sleep(Duration::from_secs(1));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(&mut simulator);
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Ensure the thread is stopped on exit
    app.stop_serial_thread();

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App<'_>) -> io::Result<()> {
    let tick_rate = Duration::from_millis(250);
    let mut last_tick = Instant::now();
    let rx = app.serial_rx.take().unwrap();

    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Ok(message) = rx.try_recv() {
            match message {
                SerialMessage::Log(msg) => app.log(msg),
                SerialMessage::Error(err) => app.log(format!("[SERIAL ERROR] {}", err)),
            }
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.mode {
                        AppMode::Menu => handle_menu_input(app, key),
                        AppMode::Manual => handle_manual_input(app, key),
                        AppMode::SerialSelect => handle_serial_select_input(app, key),
                        AppMode::SerialListen => handle_serial_listen_input(app, key),
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        if matches!(app.mode, AppMode::Exiting) {
            return Ok(());
        }
    }
}

fn handle_menu_input(app: &mut App<'_>, key: event::KeyEvent) {
    let menu_items = ["Manual Command Input", "Listen on Serial Port", "Exit"];
    match key.code {
        KeyCode::Char('q') => app.mode = AppMode::Exiting,
        KeyCode::Down => {
            app.menu_selection = (app.menu_selection + 1) % menu_items.len();
        }
        KeyCode::Up => {
            app.menu_selection = (app.menu_selection + menu_items.len() - 1) % menu_items.len();
        }
        KeyCode::Enter => match app.menu_selection {
            0 => {
                app.mode = AppMode::Manual;
                app.focus = Focus::Input;
                app.log("Entered Manual Mode.".into());
            }
            1 => {
                app.scan_ports();
                app.mode = AppMode::SerialSelect;
                app.focus = Focus::SerialPortList;
            }
            2 => app.mode = AppMode::Exiting,
            _ => {}
        },
        _ => {}
    }
}

fn handle_manual_input(app: &mut App<'_>, key: event::KeyEvent) {
    if key.code == KeyCode::Esc {
        app.mode = AppMode::Menu;
        app.focus = Focus::Menu;
        app.log("Returned to main menu.".into());
        return;
    }

    if key.code == KeyCode::Tab {
        app.focus = match app.focus {
            Focus::Input => Focus::Logs,
            Focus::Logs => Focus::Input,
            _ => Focus::Input,
        };
        return;
    }

    match app.focus {
        Focus::Input => match key.code {
            KeyCode::Char(c) if !c.is_control() => app.input.push(c),
            KeyCode::Backspace => {
                app.input.pop();
            }
            KeyCode::Enter => {
                if !app.input.is_empty() {
                    app.process_command(&app.input.clone());
                    app.input.clear();
                }
            }
            _ => {}
        },
        Focus::Logs => match key.code {
            KeyCode::Up => {
                let current = app.log_state.selected().unwrap_or(0);
                if current < app.logs.len() - 1 {
                    app.log_state.select(Some(current + 1));
                }
            }
            KeyCode::Down => {
                let current = app.log_state.selected().unwrap_or(0);
                if current > 0 {
                    app.log_state.select(Some(current - 1));
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn handle_serial_select_input(app: &mut App<'_>, key: event::KeyEvent) {
    if key.code == KeyCode::Esc {
        app.mode = AppMode::Menu;
        app.focus = Focus::Menu;
        app.log("Serial port selection cancelled.".into());
        return;
    }

    if key.code == KeyCode::Tab {
        app.focus = match app.focus {
            Focus::SerialPortList => Focus::BaudRateList,
            _ => Focus::SerialPortList,
        };
        return;
    }

    match app.focus {
        Focus::SerialPortList => {
            let list_len = app.available_ports.len();
            if list_len == 0 {
                return;
            }
            let current = app.port_list_state.selected().unwrap_or(0);
            match key.code {
                KeyCode::Up => app.port_list_state.select(Some((current + list_len - 1) % list_len)),
                KeyCode::Down => app.port_list_state.select(Some((current + 1) % list_len)),
                _ => {}
            }
        }
        Focus::BaudRateList => {
            let list_len = app.baud_rates.len();
            let current = app.baud_rate_list_state.selected().unwrap_or(0);
            match key.code {
                KeyCode::Up => app.baud_rate_list_state.select(Some((current + list_len - 1) % list_len)),
                KeyCode::Down => app.baud_rate_list_state.select(Some((current + 1) % list_len)),
                _ => {}
            }
        }
        _ => {}
    }

    if key.code == KeyCode::Enter {
        if let (Some(port_index), Some(baud_index)) = (app.port_list_state.selected(), app.baud_rate_list_state.selected()) {
            if port_index >= app.available_ports.len() { return; }
            let port_name = app.available_ports[port_index].clone();
            let baud_rate = app.baud_rates[baud_index];
            app.log(format!("Starting to listen on {} at {} baud.", port_name, baud_rate));
            app.mode = AppMode::SerialListen;
            app.focus = Focus::Logs; // Default focus to logs for scrolling

            let tx = app.serial_tx.clone();
            let mut simulator_clone = app.simulator.clone();
            let stop_flag = Arc::new(AtomicBool::new(false));
            app.serial_should_stop = Some(stop_flag.clone());

            let handle = thread::spawn(move || {
                let port = serialport::new(&port_name, baud_rate)
                    .timeout(Duration::from_millis(100))
                    .open();

                let mut port = match port {
                    Ok(p) => p,
                    Err(e) => {
                        tx.send(SerialMessage::Error(format!("Failed to open port: {}", e))).unwrap();
                        return;
                    }
                };

                let mut serial_buf: Vec<u8> = vec![0; 128];
                while !stop_flag.load(Ordering::Relaxed) {
                    match port.read(serial_buf.as_mut_slice()) {
                        Ok(bytes_read) => {
                            if bytes_read > 0 {
                                let command_str = std::str::from_utf8(&serial_buf[..bytes_read]).unwrap_or("").trim();
                                if !command_str.is_empty() {
                                    tx.send(SerialMessage::Log(format!("> {}", command_str))).unwrap();
                                    match simulator_clone.process_command(command_str.as_bytes()) {
                                        Ok(result) => {
                                            // Send any debug logs
                                            for debug_log in result.logs {
                                                tx.send(SerialMessage::Log(debug_log)).unwrap();
                                            }
                                            // Handle the actual response
                                            if let Some(response) = result.response {
                                                tx.send(SerialMessage::Log(format!("< {}", response))).unwrap();
                                                if let Err(e) = port.write_all(response.as_bytes()) {
                                                    tx.send(SerialMessage::Error(format!("Failed to write to port: {}", e))).unwrap();
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tx.send(SerialMessage::Log(format!("[ERROR] {:?}", e))).unwrap();
                                        }
                                    }
                                }
                            }
                        }
                        Err(ref e) if e.kind() == io::ErrorKind::TimedOut => continue,
                        Err(e) => {
                            tx.send(SerialMessage::Error(format!("{}", e))).unwrap();
                            break;
                        }
                    }
                }
            });
            app.serial_thread_handle = Some(handle);
        }
    }
}

fn handle_serial_listen_input(app: &mut App<'_>, key: event::KeyEvent) {
    if key.code == KeyCode::Esc {
        app.stop_serial_thread();
        app.mode = AppMode::Menu;
        app.focus = Focus::Menu;
        app.log("Stopped listening on serial port.".into());
        return;
    }

    // In this mode, the only interactive element is the log panel
    match key.code {
        KeyCode::Up => {
            let current = app.log_state.selected().unwrap_or(0);
            if current < app.logs.len() - 1 {
                app.log_state.select(Some(current + 1));
            }
        }
        KeyCode::Down => {
            let current = app.log_state.selected().unwrap_or(0);
            if current > 0 {
                app.log_state.select(Some(current - 1));
            }
        }
        _ => {}
    }
}


fn ui(f: &mut Frame, app: &mut App<'_>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Percentage(40),
                Constraint::Length(1),
            ]
                .as_ref(),
        )
        .split(f.size());

    let status_text = format!(
        "Address: 0x{:02X} | Mode: {}",
        app.simulator.rs485_address,
        match app.mode {
            AppMode::Menu => "Menu",
            AppMode::Manual => "Manual Input",
            AppMode::SerialSelect => "Serial Port Select",
            AppMode::SerialListen => "Listening on Serial",
            AppMode::Exiting => "Exiting",
        }
    );
    let status_bar = Paragraph::new(status_text)
        .style(Style::default().bg(Color::Blue).fg(Color::White))
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(status_bar, chunks[0]);

    match app.mode {
        AppMode::Menu => draw_menu(f, app, chunks[1]),
        AppMode::Manual => draw_manual_mode(f, app, chunks[1]),
        AppMode::SerialSelect => draw_serial_select(f, app, chunks[1]),
        AppMode::SerialListen => draw_serial_listen(f, app, chunks[1]),
        _ => {}
    }

    let log_messages: Vec<ListItem> = app.logs.iter().rev().map(|msg| ListItem::new(msg.as_str())).collect();
    let log_list = List::new(log_messages)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Logs")
                .border_style(if matches!(app.focus, Focus::Logs) || matches!(app.mode, AppMode::SerialListen) {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }),
        )
        .direction(ListDirection::BottomToTop)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::DarkGray))
        .highlight_symbol(">> ");
    f.render_stateful_widget(log_list, chunks[2], &mut app.log_state);

    let footer_text = match app.mode {
        AppMode::Menu => "Use ↑/↓ to navigate, Enter to select, 'q' to quit.",
        AppMode::Manual => match app.focus {
            Focus::Input => "Type command, Enter to send, Tab to focus logs, Esc for menu.",
            Focus::Logs => "Use ↑/↓ to scroll logs, Tab to focus input, Esc for menu.",
            _ => "Esc to return to menu.",
        },
        AppMode::SerialSelect => "Use ↑/↓ to navigate, Tab to switch panels, Enter to confirm, Esc to cancel.",
        AppMode::SerialListen => "Listening... Use ↑/↓ to scroll logs, Esc to stop and return to menu.",
        _ => "'q' to quit.",
    };
    let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::Cyan));
    f.render_widget(footer, chunks[3]);
}

fn draw_menu(f: &mut Frame, app: &mut App<'_>, area: Rect) {
    let menu_items = ["Manual Command Input", "Listen on Serial Port", "Exit"];
    let list_items: Vec<ListItem> = menu_items.iter().map(|&i| ListItem::new(i)).collect();

    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title("Main Menu"))
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol(">> ");

    let mut list_state = ListState::default();
    list_state.select(Some(app.menu_selection));

    f.render_stateful_widget(list, area, &mut list_state);
}

fn draw_manual_mode(f: &mut Frame, app: &mut App<'_>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(area);

    let input_paragraph = Paragraph::new(app.input.as_str()).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Command Input")
            .border_style(if matches!(app.focus, Focus::Input) {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            }),
    );
    f.render_widget(input_paragraph, chunks[0]);

    if matches!(app.focus, Focus::Input) {
        f.set_cursor(chunks[0].x + app.input.len() as u16 + 1, chunks[0].y + 1);
    }

    let instructions = Paragraph::new("Enter commands in the box above.\nExample: <C1F21>\nPress Esc to return to the main menu.")
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Info"));
    f.render_widget(instructions, chunks[1]);
}

fn draw_serial_select(f: &mut Frame, app: &mut App<'_>, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let port_items: Vec<ListItem> = app.available_ports.iter().map(|p| ListItem::new(p.as_str())).collect();
    let port_list = List::new(port_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Serial Ports")
                .border_style(if app.focus == Focus::SerialPortList {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol(">> ");
    f.render_stateful_widget(port_list, chunks[0], &mut app.port_list_state);

    let baud_items: Vec<ListItem> = app.baud_rates.iter().map(|b| ListItem::new(b.to_string())).collect();
    let baud_list = List::new(baud_items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Baud Rate")
                .border_style(if app.focus == Focus::BaudRateList {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                }),
        )
        .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
        .highlight_symbol(">> ");
    f.render_stateful_widget(baud_list, chunks[1], &mut app.baud_rate_list_state);
}

fn draw_serial_listen(f: &mut Frame, app: &mut App<'_>, area: Rect) {
    let port_name = app.port_list_state.selected().map_or("N/A".to_string(), |i| app.available_ports.get(i).cloned().unwrap_or_default());
    let baud_rate = app.baud_rate_list_state.selected().map_or(0, |i| app.baud_rates[i]);

    let text = vec![
        Line::from(""),
        Line::from(Span::styled("Listening on Serial Port", Style::default().add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(format!("  Port: {}", port_name)),
        Line::from(format!("  Baud: {}", baud_rate)),
        Line::from(""),
        Line::from("Check logs below for incoming data."),
    ];

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Serial Monitor"));
    f.render_widget(paragraph, area);
}
