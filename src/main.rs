use ez_sim::{CommandError, Simulator};
use std::io::{self, Write};

fn main() {
    // The RS-485 address for our simulator. Let's use 0x1F as an example.
    const SIMULATOR_ADDRESS: u8 = 0x1F;

    // Create an instance of our simulator from the library.
    let mut simulator = Simulator::new(SIMULATOR_ADDRESS);

    println!("Endzone 250 Simulator");
    println!("Address: 0x{:02X}", SIMULATOR_ADDRESS);
    println!("Enter commands, or type 'exit' to quit.");

    // Main loop to read and process commands.
    loop {
        print!("> ");
        io::stdout().flush().unwrap(); // Ensure the prompt is displayed.

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                let command = input.trim();
                if command == "exit" {
                    break;
                }

                if command.is_empty() {
                    continue;
                }

                // Pass the command as a byte slice to our library and handle the result.
                match simulator.process_command(command.as_bytes()) {
                    Ok(Some(response)) => {
                        println!("< {}", response);
                    }
                    Ok(None) => {
                        // Command was valid but for another address or a silent data command, so we do nothing.
                    }
                    Err(e) => {
                        // The command was malformed or invalid. Print a helpful error.
                        match e {
                            CommandError::InvalidFrame => eprintln!("[ERROR] Invalid command frame. A valid command must be enclosed in '<...>'.") ,
                            CommandError::TooShort => eprintln!("[ERROR] Command content is too short."),
                            CommandError::InvalidAddress(_) => eprintln!("[ERROR] Invalid hexadecimal address in command."),
                            CommandError::InvalidCommandId(_) => eprintln!("[ERROR] Command ID is not a valid number."),
                            CommandError::UnimplementedCommand(id) => eprintln!("[ERROR] Command '{}' is not yet implemented.", id),
                            CommandError::InvalidParameter => eprintln!("[ERROR] Command contains an invalid parameter."),
                        }
                    }
                }
            }
            Err(error) => {
                eprintln!("[ERROR] Failed to read line: {}", error);
                break;
            }
        }
    }
}
