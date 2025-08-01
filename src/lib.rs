//! # Endzone 250 Simulator Library
//!
//! This library contains the core logic for simulating the Endzone 250 driver board.
//! It manages the internal state of the simulated hardware and processes commands
//! to modify that state, returning responses identical to the real hardware.

use std::num::ParseIntError;

// Custom error types for command processing.
#[derive(Debug, PartialEq)]
pub enum CommandError {
    /// Command is missing a valid '<...>' frame.
    InvalidFrame,
    /// Command content is too short to be valid.
    TooShort,
    /// The address portion of the command is not valid hexadecimal.
    InvalidAddress(ParseIntError),
    /// The command ID is not a valid number.
    InvalidCommandId(ParseIntError),
    /// The command ID is known, but not yet implemented.
    UnimplementedCommand(u8),
    /// The command is known, but has an invalid parameter.
    InvalidParameter,
}

// Represents all possible numeric commands from the C firmware.
#[derive(Debug, PartialEq)]
enum Command {
    SequenceOn,
    SequenceOff,
    // Command 50 has several sub-modes for data loading.
    DataLoad(DataLoadMode),
    // ... other commands will be added here
}

#[derive(Debug, PartialEq)]
enum DataLoadMode {
    StartPatternLoad,
    EndPatternLoad,
    StartDriverConfigLoad,
    EndDriverConfigLoad,
}

// Represents the state of a single Power Supply Unit (PSU).
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Psu {
    pub enabled: bool,
    pub voltage_setpoint: f32,
    pub current_limit: f32,
    // Voltage settings for different steps
    pub voltage_set_s1: u16,
    pub voltage_set_s2: u16,
    pub voltage_set_s3: u16,
    pub voltage_set_s4: u16,
    // High and low voltage monitor limits
    pub high_voltage_limit: f32,
    pub low_voltage_limit: f32,
    // Current monitor limit and calibration
    pub current_monitor_limit: f32,
    pub i_cal_val: f32,
    pub i_cal_offset_val: f32,
    pub pos_neg_i: u32,
    // Voltage calibration offset
    pub v_cal_offset_val: f32,
    pub pos_neg_v: u32,
    // Power-on sequence configuration
    pub sequence_id: u8,
    pub sequence_delay: u32,
}

// Represents the state of an FPGA, including its pattern memory.
#[derive(Debug, Default, Clone)]
pub struct Fpga {
    pub present: bool,
    // Using a Vec<u8> to represent pattern memory.
    // This can be adapted to a more complex structure if needed.
    pub pattern_memory: Vec<u8>,
}

// Represents the state of a Clock Generator module.
#[derive(Debug, Default, Clone)]
pub struct ClockGenerator {
    pub present: bool,
    pub enabled: bool,
    pub frequency: u32,
}

// Represents the state of a Sine Wave generator module.
#[derive(Debug, Default, Clone)]
pub struct SineWave {
    pub present: bool,
    pub enabled: bool,
    pub frequency: u32,
    pub amplitude: u16,
    pub offset: u16,
}

// The main struct that holds the entire state of the simulated driver board.
#[derive(Debug, Clone)]
pub struct Simulator {
    // The 2-character hexadecimal RS-485 address of the simulator.
    pub rs485_address: u8,
    // An array of 6 PSUs, as suggested by the C code (PSU_1_DATA to PSU_6_DATA).
    pub psus: [Psu; 6],
    // Two FPGAs are mentioned in the C code (FPGA1_Present, FPGA2_Present).
    pub fpgas: [Fpga; 2],
    // Four Clock Generators (CLKMOD1_Present to CLKMOD4_Present).
    pub clock_generators: [ClockGenerator; 4],
    // Two Sine Wave modules (SW1_Present, SW2_Present).
    pub sine_waves: [SineWave; 2],
    // Timer and Alarm values
    pub timer_values: [u32; 4],
    pub alarm_values: [u32; 4],
    // --- Internal state for data loading sessions ---
    sram_address: u32,
    pattern_data_checksum: u32,
    driver_data_checksum: u32,
    is_pattern_data_loading: bool,
    is_driver_data_loading: bool,
}

impl Simulator {
    /// Creates a new `Simulator` instance with a given RS-485 address.
    pub fn new(rs485_address: u8) -> Self {
        Self {
            rs485_address,
            psus: Default::default(),
            fpgas: Default::default(),
            clock_generators: Default::default(),
            sine_waves: Default::default(),
            timer_values: [0; 4],
            alarm_values: [0; 4],
            sram_address: 1,
            pattern_data_checksum: 0,
            driver_data_checksum: 0,
            is_pattern_data_loading: false,
            is_driver_data_loading: false,
        }
    }

    /// Parses the content of a command string into a `Command` enum.
    fn parse_command(&self, content: &str) -> Result<Command, CommandError> {
        let cmd_id_str = &content[3..5];
        let cmd_id = u8::from_str_radix(cmd_id_str, 10).map_err(CommandError::InvalidCommandId)?;

        match cmd_id {
            3 => Ok(Command::SequenceOn),
            4 => Ok(Command::SequenceOff),
            50 => {
                // Command 50 has a sub-mode parameter
                if content.len() < 7 {
                    return Err(CommandError::TooShort);
                }
                let param_str = &content[5..7];
                let param = u8::from_str_radix(param_str, 10).map_err(|_| CommandError::InvalidParameter)?;
                match param {
                    0 => Ok(Command::DataLoad(DataLoadMode::StartPatternLoad)),
                    1 => Ok(Command::DataLoad(DataLoadMode::EndPatternLoad)),
                    2 => Ok(Command::DataLoad(DataLoadMode::StartDriverConfigLoad)),
                    3 => Ok(Command::DataLoad(DataLoadMode::EndDriverConfigLoad)),
                    _ => Err(CommandError::InvalidParameter),
                }
            }
            _ => Err(CommandError::UnimplementedCommand(cmd_id)),
        }
    }

    /// Processes a command string and returns the appropriate response.
    pub fn process_command(&mut self, command_str: &str) -> Result<Option<String>, CommandError> {
        let start_byte = command_str.find('<');
        let end_byte = command_str.find('>');

        let content = match (start_byte, end_byte) {
            (Some(start), Some(end)) if end > start => &command_str[start + 1..end],
            _ => return Err(CommandError::InvalidFrame),
        };

        // Handle data loading commands first if a session is active.
        if self.is_driver_data_loading {
            if let Some(cmd_char) = content.chars().next() {
                match cmd_char {
                    'V' => {
                        self.handle_v_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'Q' => {
                        self.handle_q_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'T' => {
                        self.handle_t_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'D' => {
                        self.handle_d_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    // 'E', etc. will be handled here
                    _ => {} // Fall through to 'C' command check
                }
            }
        }

        if content.len() < 5 {
            return Err(CommandError::TooShort);
        }

        // Handle 'C' type control commands
        if &content[0..1] == "C" {
            let addr_str = &content[1..3];
            let address = u8::from_str_radix(addr_str, 16).map_err(CommandError::InvalidAddress)?;

            if address != self.rs485_address {
                return Ok(None); // Silently ignore
            }

            // Parse the command and dispatch it
            let command = self.parse_command(content)?;
            let response = self.execute_command(command);
            return Ok(Some(response));
        }

        Ok(None)
    }

    /// Executes a parsed command and returns the response string.
    fn execute_command(&mut self, command: Command) -> String {
        match command {
            Command::SequenceOn => {
                // TODO: Implement logic to turn on PSUs, clocks, etc.
                String::from("#ON#")
            }
            Command::SequenceOff => {
                // TODO: Implement logic to turn off all systems.
                String::from("#OFF#")
            }
            Command::DataLoad(mode) => match mode {
                DataLoadMode::StartPatternLoad => {
                    self.is_pattern_data_loading = true;
                    self.is_driver_data_loading = false;
                    self.sram_address = 1;
                    self.pattern_data_checksum = 0;
                    String::from("#OK#")
                }
                DataLoadMode::EndPatternLoad => {
                    self.is_pattern_data_loading = false;
                    format!("#{},{},#", self.pattern_data_checksum, self.sram_address)
                }
                DataLoadMode::StartDriverConfigLoad => {
                    self.is_driver_data_loading = true;
                    self.is_pattern_data_loading = false;
                    self.driver_data_checksum = 0;
                    String::from("#OK#")
                }
                DataLoadMode::EndDriverConfigLoad => {
                    self.is_driver_data_loading = false;
                    format!("#{}#", self.driver_data_checksum)
                }
            },
        }
    }

    /// Parses a 'V' command and updates the driver data checksum.
    fn handle_v_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram6 = parse_hex(3, 5)?;
        let sram5 = parse_hex(5, 7)?;
        let sram4 = parse_hex(7, 10)?;
        let sram3 = parse_hex(10, 13)?;
        let sram2 = parse_hex(13, 16)?;
        let sram1 = parse_hex(16, 19)?;

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6;
        Ok(())
    }

    /// Parses a 'Q' command, updates PSU state, and updates the checksum.
    fn handle_q_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 21 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram6_psu_num = parse_hex(3, 5)? as usize;
        let sram5_delay = parse_hex(5, 8)?;
        let sram4_seq_id = parse_hex(8, 9)? as u8;
        let sram3_cal_v = parse_hex(9, 13)?;
        let sram2_low_v = parse_hex(13, 16)?;
        let sram1_high_v = parse_hex(16, 19)?;
        let sram8_vmon_mult = parse_hex(20, 21)?;

        // PSU number in C code is 1-based, our array is 0-based.
        if sram6_psu_num > 0 && sram6_psu_num <= self.psus.len() {
            let psu = &mut self.psus[sram6_psu_num - 1];
            psu.sequence_id = sram4_seq_id;
            psu.sequence_delay = sram5_delay;

            let vmon_divisor = if sram8_vmon_mult == 1 { 1.0 } else { 10.0 };
            psu.high_voltage_limit = sram1_high_v as f32 / vmon_divisor;
            psu.low_voltage_limit = sram2_low_v as f32 / vmon_divisor;
        }

        self.driver_data_checksum += sram1_high_v + sram2_low_v + sram3_cal_v + sram4_seq_id as u32 + sram5_delay + sram6_psu_num as u32;
        Ok(())
    }

    /// Parses a 'T' command, updates timer state, and updates the checksum.
    fn handle_t_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram8 = parse_hex(3, 5)?;
        let sram7 = parse_hex(5, 7)?;
        let sram6 = parse_hex(7, 9)?;
        let sram5 = parse_hex(9, 11)?;
        let sram4 = parse_hex(11, 13)?;
        let sram3 = parse_hex(13, 15)?;
        let sram2 = parse_hex(15, 17)?;
        let sram1 = parse_hex(17, 19)?;

        self.timer_values[0] = sram1;
        self.timer_values[1] = sram2;
        self.timer_values[2] = sram3;
        self.timer_values[3] = sram4;
        self.alarm_values[0] = sram5;
        self.alarm_values[1] = sram6;
        self.alarm_values[2] = sram7;
        self.alarm_values[3] = sram8;

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8;
        Ok(())
    }

    /// Parses a 'D' command, updates PSU state, and updates the checksum.
    fn handle_d_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 17 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram3_psu_num = parse_hex(3, 5)? as usize;
        let sram2_i_cal = parse_hex(5, 9)?;
        let sram1_i_mon = parse_hex(9, 12)?;
        let sram4_i_cal_off = parse_hex(12, 16)?;
        let sram5_pos_neg = parse_hex(16, 17)?;

        if sram3_psu_num > 0 && sram3_psu_num < 7 {
            // Standard PSU current config
            let psu = &mut self.psus[sram3_psu_num - 1];
            psu.current_monitor_limit = sram1_i_mon as f32 / 100.0;
            psu.i_cal_val = sram2_i_cal as f32 / 1000.0;
            psu.i_cal_offset_val = sram4_i_cal_off as f32 / 100.0;
            psu.pos_neg_i = sram5_pos_neg;
            if psu.pos_neg_i == 1 {
                psu.i_cal_offset_val *= -1.0;
            }
        } else if sram3_psu_num >= 7 && sram3_psu_num < 9 {
            // Special case for voltage offset config
            let target_psu_index = sram3_psu_num - 7; // 7 -> 0, 8 -> 1
            let psu = &mut self.psus[target_psu_index];
            psu.v_cal_offset_val = sram4_i_cal_off as f32 / 100.0;
            psu.pos_neg_v = sram5_pos_neg;
            if psu.pos_neg_v == 1 {
                psu.v_cal_offset_val *= -1.0;
            }
        }

        self.driver_data_checksum += sram1_i_mon + sram2_i_cal + sram3_psu_num as u32 + sram4_i_cal_off + sram5_pos_neg;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Tests for basic parsing and addressing ---

    #[test]
    fn simulator_creation() {
        let sim = Simulator::new(0x2A);
        assert_eq!(sim.rs485_address, 0x2A);
    }

    #[test]
    fn process_valid_command() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command("<C1F03>").unwrap();
        assert_eq!(response, Some(String::from("#ON#")));
    }

    #[test]
    fn process_command_with_trailing_characters() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command("<C1F03>>>garbage").unwrap();
        assert_eq!(response, Some(String::from("#ON#")));
    }

    #[test]
    fn process_command_with_leading_characters() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command("noise<C1F03>").unwrap();
        assert_eq!(response, Some(String::from("#ON#")));
    }

    #[test]
    fn ignore_command_for_other_address() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command("<C2A03>").unwrap();
        assert_eq!(response, None);
    }

    #[test]
    fn reject_malformed_frame() {
        let mut sim = Simulator::new(0x1F);
        assert_eq!(sim.process_command("C1F03>").unwrap_err(), CommandError::InvalidFrame);
        assert_eq!(sim.process_command("<C1F03").unwrap_err(), CommandError::InvalidFrame);
        assert_eq!(sim.process_command(">C1F03<").unwrap_err(), CommandError::InvalidFrame);
    }

    #[test]
    fn reject_too_short_command() {
        let mut sim = Simulator::new(0x1F);
        assert_eq!(sim.process_command("<C1F>").unwrap_err(), CommandError::TooShort);
    }

    #[test]
    fn reject_invalid_hex_address() {
        let mut sim = Simulator::new(0x1F);
        let result = sim.process_command("<CZZ03>");
        assert!(matches!(result, Err(CommandError::InvalidAddress(_))));
    }

    // --- Tests for specific command logic ---

    #[test]
    fn process_command_50_pattern_load_cycle() {
        let mut sim = Simulator::new(0x1F);
        let response1 = sim.process_command("<C1F5000>").unwrap();
        assert_eq!(response1, Some(String::from("#OK#")));
        let response2 = sim.process_command("<C1F5001>").unwrap();
        assert_eq!(response2, Some(String::from("#0,1,#")));
    }

    #[test]
    fn process_command_50_driver_load_cycle() {
        let mut sim = Simulator::new(0x1F);
        let response1 = sim.process_command("<C1F5002>").unwrap();
        assert_eq!(response1, Some(String::from("#OK#")));
        let response2 = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(response2, Some(String::from("#0#")));
    }

    #[test]
    fn process_sequence_on_off_commands() {
        let mut sim = Simulator::new(0x1F);
        let response_on = sim.process_command("<C1F03>").unwrap();
        assert_eq!(response_on, Some(String::from("#ON#")));
        let response_off = sim.process_command("<C1F04>").unwrap();
        assert_eq!(response_off, Some(String::from("#OFF#")));
    }

    #[test]
    fn checksum_validation_during_driver_load() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();
        let v_command = "<Vxx0605004003002001>";
        let expected_checksum = 0x06 + 0x05 + 0x004 + 0x003 + 0x002 + 0x001;
        sim.process_command(v_command).unwrap();
        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn q_command_updates_psu_state_and_checksum() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();
        let q_command = "<Qxx0306420C8007D0FA00>";
        let psu_num = 0x03; let delay = 0x064; let seq_id = 0x2;
        let cal_v = 0x0C80; let low_v = 0x07D; let high_v = 0x0FA;
        let expected_checksum = psu_num + delay + seq_id + cal_v + low_v + high_v;
        sim.process_command(q_command).unwrap();
        let psu = &sim.psus[2];
        assert_eq!(psu.sequence_id, 2);
        assert_eq!(psu.sequence_delay, 100);
        assert_eq!(psu.high_voltage_limit, 25.0);
        assert_eq!(psu.low_voltage_limit, 12.5);
        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn t_command_updates_timer_and_checksum() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();
        let t_command = "<Txx0807060504030201>";
        let s1 = 0x01; let s2 = 0x02; let s3 = 0x03; let s4 = 0x04;
        let s5 = 0x05; let s6 = 0x06; let s7 = 0x07; let s8 = 0x08;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;
        sim.process_command(t_command).unwrap();
        assert_eq!(sim.timer_values, [s1, s2, s3, s4]);
        assert_eq!(sim.alarm_values, [s5, s6, s7, s8]);
        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn d_command_updates_psu_current_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // D<psu=04><i_cal=3E80><i_mon=C80><i_cal_off=0641><pos_neg=1>
        let d_command = "<Dxx043E80C8006411>";

        let psu_num = 0x04;
        let i_cal = 0x3E80;
        let i_mon = 0xC80;
        let i_cal_off = 0x0641;
        let pos_neg = 1;
        let expected_checksum = psu_num + i_cal + i_mon + i_cal_off + pos_neg;

        sim.process_command(d_command).unwrap();

        let psu = &sim.psus[3]; // PSU #4 is at index 3
        assert_eq!(psu.current_monitor_limit, 32.0); // 0xC80 = 3200 -> 32.00
        assert_eq!(psu.i_cal_val, 16.0); // 0x3E80 = 16000 -> 16.000
        assert_eq!(psu.i_cal_offset_val, -16.01); // 0x641 = 1601 -> 16.01, pos_neg=1 makes it negative
        assert_eq!(psu.pos_neg_i, 1);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn d_command_updates_psu_voltage_offset() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // D<psu=07><...><v_cal_off=0032><pos_neg=0>
        // PSU #7 maps to voltage offset for PSU #1 (index 0)
        let d_command = "<Dxx07000000000320>";

        let psu_num = 0x07;
        let i_cal = 0x0;
        let i_mon = 0x0;
        let v_cal_off = 0x0032;
        let pos_neg = 0;
        let expected_checksum = psu_num + i_cal + i_mon + v_cal_off + pos_neg;

        sim.process_command(d_command).unwrap();

        let psu = &sim.psus[0]; // Target is PSU #1 (index 0)
        assert_eq!(psu.v_cal_offset_val, 0.5); // 0x32 = 50 -> 0.50
        assert_eq!(psu.pos_neg_v, 0);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }
}
