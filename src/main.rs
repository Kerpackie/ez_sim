use ez_sim::{CommandError, Simulator};
use std::io::{self, BufRead, Write};
use std::time::Duration;

// The main entry point for the command-line simulator application.
fn main() {
    println!("=========================");
    println!("  Endzone 250 Simulator  ");
    println!("=========================");

    // Prompt the user for the simulator's address.
    print!("Enter RS-485 address (hex, default: 1F): ");
    io::stdout().flush().unwrap();

    let mut addr_input = String::new();
    io::stdin().read_line(&mut addr_input).unwrap();

    // Parse the input or use the default value.
    let simulator_address = match addr_input.trim() {
        "" => 0x1F, // Default address
        s => u8::from_str_radix(s, 16).unwrap_or_else(|_| {
            eprintln!("[WARNING] Invalid hex address '{}'. Using default 0x1F.", s);
            0x1F
        }),
    };

    // Create an instance of our simulator from the library with the chosen address.
    let mut simulator = Simulator::new(simulator_address);

    println!("Simulator started with Address: 0x{:02X}", simulator_address);

    // Main menu loop.
    loop {
        println!("\nSelect mode:");
        println!("  1. Manual Command Input");
        println!("  2. Listen on Serial Port");
        println!("  3. Exit");
        print!("> ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();

        match choice.trim() {
            "1" => run_manual_mode(&mut simulator),
            "2" => run_serial_mode(&mut simulator),
            "3" => break,
            _ => eprintln!("[ERROR] Invalid choice. Please enter 1, 2, or 3."),
        }
    }
}

// Handles the manual command input mode.
fn run_manual_mode(simulator: &mut Simulator) {
    println!("\n--- Manual Mode ---");
    println!("Enter commands, or type 'back' to return to the main menu.");
    print!("> ");
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let input = line.unwrap();
        let command = input.trim();

        if command == "back" {
            break;
        }

        if command.is_empty() {
            print!("> ");
            io::stdout().flush().unwrap();
            continue;
        }

        process_and_display_command(simulator, command);
        print!("> ");
        io::stdout().flush().unwrap();
    }
}

// Handles the serial port listening mode.
fn run_serial_mode(simulator: &mut Simulator) {
    println!("\n--- Serial Mode ---");

    // List available serial ports.
    let ports = match serialport::available_ports() {
        Ok(ports) => ports,
        Err(e) => {
            eprintln!("[ERROR] Could not enumerate serial ports: {}", e);
            return;
        }
    };

    if ports.is_empty() {
        eprintln!("[ERROR] No serial ports found.");
        return;
    }

    println!("Available serial ports:");
    for (i, port) in ports.iter().enumerate() {
        println!("  {}: {}", i, port.port_name);
    }

    // Get user's choice of serial port.
    print!("Select a port (number): ");
    io::stdout().flush().unwrap();
    let mut port_choice = String::new();
    io::stdin().read_line(&mut port_choice).unwrap();
    let port_index: usize = match port_choice.trim().parse() {
        Ok(i) if i < ports.len() => i,
        _ => {
            eprintln!("[ERROR] Invalid port selection.");
            return;
        }
    };
    let port_name = &ports[port_index].port_name;

    // Get user's choice of baud rate.
    let baud_rates = [9600, 19200, 38400, 57600, 115200];
    println!("Available baud rates:");
    for (i, &rate) in baud_rates.iter().enumerate() {
        println!("  {}: {}", i, rate);
    }
    print!("Select a baud rate (number): ");
    io::stdout().flush().unwrap();
    let mut baud_choice = String::new();
    io::stdin().read_line(&mut baud_choice).unwrap();
    let baud_index: usize = match baud_choice.trim().parse() {
        Ok(i) if i < baud_rates.len() => i,
        _ => {
            eprintln!("[ERROR] Invalid baud rate selection.");
            return;
        }
    };
    let baud_rate = baud_rates[baud_index];

    // Open the selected serial port.
    let mut port = match serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
    {
        Ok(port) => port,
        Err(e) => {
            eprintln!("[ERROR] Failed to open port '{}': {}", port_name, e);
            return;
        }
    };

    println!(
        "\nListening on {} at {} baud. Press Ctrl+C to exit.",
        port_name, baud_rate
    );

    let mut serial_buf: Vec<u8> = vec![0; 128];
    loop {
        match port.read(serial_buf.as_mut_slice()) {
            Ok(bytes_read) => {
                if bytes_read > 0 {
                    let command_str = std::str::from_utf8(&serial_buf[..bytes_read])
                        .unwrap_or("")
                        .trim();
                    if !command_str.is_empty() {
                        println!("> Received: {}", command_str);
                        let response = process_and_display_command(simulator, command_str);
                        if let Some(res) = response {
                            if let Err(e) = port.write_all(res.as_bytes()) {
                                eprintln!("[ERROR] Failed to write to serial port: {}", e);
                            }
                        }
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::TimedOut => (),
            Err(e) => eprintln!("[ERROR] Serial port error: {}", e),
        }
    }
}

// Common function to process a command string and print the output.
fn process_and_display_command(simulator: &mut Simulator, command: &str) -> Option<String> {
    match simulator.process_command(command.as_bytes()) {
        Ok(Some(response)) => {
            println!("< {}", response);
            Some(response)
        }
        Ok(None) => None,
        Err(e) => {
            match e {
                CommandError::InvalidFrame => eprintln!("[ERROR] Invalid command frame. A valid command must be enclosed in '<...>'.") ,
                CommandError::TooShort => eprintln!("[ERROR] Command content is too short."),
                CommandError::InvalidAddress(_) => eprintln!("[ERROR] Invalid hexadecimal address in command."),
                CommandError::InvalidCommandId(_) => eprintln!("[ERROR] Command ID is not a valid number."),
                CommandError::UnimplementedCommand(id) => eprintln!("[ERROR] Command '{}' is not yet implemented.", id),
                CommandError::InvalidParameter => eprintln!("[ERROR] Command contains an invalid parameter."),
            }
            None
        }
    }
}
