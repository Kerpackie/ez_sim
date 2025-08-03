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
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SineWave {
    pub present: bool,
    pub enabled: bool,
    pub amplitude: u32,
    pub offset: u32,
    pub frequency_base: u32,
    pub duty_cycle: u32,
    pub reset_value: u32,
}

// Represents system-wide configuration and error handling settings.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SystemConfig {
    pub auto_reset: bool,
    pub auto_reset_retries: u32,
    pub stop_on_v_error: bool,
    pub stop_on_i_error: bool,
    pub stop_on_clk_error: bool,
    pub psu_sequence_enabled: bool,
    pub stop_on_temp_error: bool,
    pub psu_step_enabled: bool,
    pub psu_step_delay: u32,
    pub power_up_delay: u32,
    pub set_point_enabled: bool,
    // Clock configuration
    pub clocks_required: bool,
    pub clocks_restart_required: bool,
    pub clocks_restart_time: u32,
    pub clk32_mon_filter: u32,
    pub clk64_mon_filter: u32,
    // Sequence delays
    pub seq_on_delay_1: u32,
    pub seq_off_delay_1: u32,
    pub seq_on_delay_2: u32,
    pub seq_off_delay_2: u32,
    pub seq_on_delay_3: u32,
    pub seq_off_delay_3: u32,
    pub sigs_mod_sequence_on: u32,
    pub sigs_mod_sequence_off: u32,
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
    // System configuration
    pub system_config: SystemConfig,
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
            system_config: Default::default(),
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
                    'S' => {
                        self.handle_s_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'E' => {
                        self.handle_e_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'A' => {
                        self.handle_a_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'F' => {
                        self.handle_f_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
                    'J' => {
                        self.handle_j_command(content)?;
                        return Ok(None); // Data commands are silent
                    }
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

    /// Parses an 'S' command, updates Sine Wave state, and updates the checksum.
    fn handle_s_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram8_sw_num = parse_hex(3, 5)? as usize;
        let sram7_used = parse_hex(5, 6)?;
        let sram6_type = parse_hex(6, 7)?;
        let sram5_reset = parse_hex(7, 9)?;
        let sram4_duty = parse_hex(9, 11)?;
        let sram3_freq_base = parse_hex(11, 13)?;
        let sram2_offset = parse_hex(13, 16)?;
        let sram1_amp = parse_hex(16, 19)?;

        if sram8_sw_num > 0 && sram8_sw_num <= self.sine_waves.len() {
            let sw = &mut self.sine_waves[sram8_sw_num - 1];
            sw.enabled = sram7_used == 1;
            sw.reset_value = sram5_reset;
            sw.duty_cycle = sram4_duty;
            sw.frequency_base = sram3_freq_base;
            sw.offset = sram2_offset;
            sw.amplitude = sram1_amp;
        }

        self.driver_data_checksum += sram1_amp + sram2_offset + sram3_freq_base + sram4_duty + sram5_reset + sram6_type + sram7_used + sram8_sw_num as u32;
        Ok(())
    }

    /// Parses an 'E' command, updates system config, and updates the checksum.
    fn handle_e_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram9 = parse_hex(3, 7)?;
        let sram8 = parse_hex(7, 9)?;
        let sram7 = parse_hex(9, 11)?;
        let sram6 = parse_hex(11, 13)?;
        let sram5 = parse_hex(13, 15)?;
        let sram4 = parse_hex(15, 16)?;
        let sram3 = parse_hex(16, 17)?;
        let sram2 = parse_hex(17, 18)?;
        let sram1 = parse_hex(18, 19)?;

        self.system_config.auto_reset = sram6 == 1;
        self.system_config.auto_reset_retries = sram7;
        self.system_config.stop_on_v_error = sram1 == 1;
        self.system_config.stop_on_i_error = sram2 == 1;
        self.system_config.stop_on_clk_error = sram3 == 1;
        self.system_config.psu_sequence_enabled = sram4 == 1;
        self.system_config.stop_on_temp_error = sram5 == 1;
        self.system_config.psu_step_enabled = sram8 == 1;
        self.system_config.psu_step_delay = sram9;

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8 + sram9;
        Ok(())
    }

    /// Parses an 'A' command, updates system config, and updates the checksum.
    fn handle_a_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram1 = parse_hex(7, 11)?;
        let sram2 = parse_hex(4, 7)?;
        let sram3 = parse_hex(3, 4)?;
        let _sram4 = parse_hex(11, 13)?; // This value is parsed but not used in the checksum.
        let sram5 = parse_hex(15, 19)?;
        let sram6 = parse_hex(14, 15)?;
        let sram7 = parse_hex(17, 19)?; // C bug: re-parses last 2 digits of sram5

        // Only a subset of parsed values are used to update state.
        self.system_config.power_up_delay = sram5;
        self.system_config.set_point_enabled = sram6 == 1;

        // The C code checksum includes the buggy sram7 but not sram4.
        self.driver_data_checksum += sram1 + sram2 + sram3 + sram5 + sram6 + sram7;
        Ok(())
    }

    /// Parses an 'F' command, updates clock config, and updates the checksum.
    fn handle_f_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 18 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram9 = parse_hex(3, 4)?;
        let sram8 = parse_hex(4, 5)?;
        let sram7 = parse_hex(5, 7)?;
        let sram6 = parse_hex(7, 9)?;
        let _sram5 = parse_hex(9, 10)?;
        let sram4 = parse_hex(10, 12)?;
        let sram3 = parse_hex(12, 14)?;
        let sram2 = parse_hex(14, 16)?;
        let sram1 = parse_hex(16, 18)?;

        self.system_config.clocks_restart_required = sram8 == 1;
        self.system_config.clocks_restart_time = (sram6 + (sram7 << 8)) * 60;
        self.system_config.clk32_mon_filter = !(sram1 + (sram2 << 8));
        self.system_config.clk64_mon_filter = !(sram3 + (sram4 << 8));
        self.system_config.clocks_required = sram9 == 1;

        // The C code's checksum for 'F' is character-by-character.
        let checksum_chars = &content[3..18];
        self.driver_data_checksum += checksum_chars.chars().fold(0, |acc, c| {
            acc + c.to_digit(16).unwrap_or(0)
        });
        Ok(())
    }

    /// Parses a 'J' command, updates sequence delays, and updates the checksum.
    fn handle_j_command(&mut self, content: &str) -> Result<(), CommandError> {
        if content.len() < 17 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram1 = parse_hex(3, 4)?;
        let sram2 = parse_hex(4, 5)?;
        let sram3 = parse_hex(5, 7)?;
        let sram4 = parse_hex(7, 9)?;
        let sram5 = parse_hex(9, 11)?;
        let sram6 = parse_hex(11, 13)?;
        let sram7 = parse_hex(13, 15)?;
        let sram8 = parse_hex(15, 17)?;

        self.system_config.sigs_mod_sequence_on = sram1;
        self.system_config.sigs_mod_sequence_off = sram2;
        self.system_config.seq_off_delay_3 = sram3;
        self.system_config.seq_on_delay_3 = sram4;
        self.system_config.seq_off_delay_2 = sram5;
        self.system_config.seq_on_delay_2 = sram6;
        self.system_config.seq_off_delay_1 = sram7;
        self.system_config.seq_on_delay_1 = sram8;

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8;
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
        let d_command = "<Dxx043E80C8006411>";
        let psu_num = 0x04; let i_cal = 0x3E80; let i_mon = 0xC80;
        let i_cal_off = 0x0641; let pos_neg = 1;
        let expected_checksum = psu_num + i_cal + i_mon + i_cal_off + pos_neg;
        sim.process_command(d_command).unwrap();
        let psu = &sim.psus[3];
        assert_eq!(psu.current_monitor_limit, 32.0);
        assert_eq!(psu.i_cal_val, 16.0);
        assert_eq!(psu.i_cal_offset_val, -16.01);
        assert_eq!(psu.pos_neg_i, 1);
        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn d_command_updates_psu_voltage_offset() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();
        let d_command = "<Dxx07000000000320>";
        let psu_num = 0x07; let i_cal = 0x0; let i_mon = 0x0;
        let v_cal_off = 0x0032; let pos_neg = 0;
        let expected_checksum = psu_num + i_cal + i_mon + v_cal_off + pos_neg;
        sim.process_command(d_command).unwrap();
        let psu = &sim.psus[0];
        assert_eq!(psu.v_cal_offset_val, 0.5);
        assert_eq!(psu.pos_neg_v, 0);
        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn s_command_updates_sine_wave_state() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // S<sw_num=01><used=1><type=0><reset=0A><duty=14><freq=03><offset=190><amp=258>
        let s_command = "<Sxx01100A1403190258>";

        let s1 = 0x258; let s2 = 0x190; let s3 = 0x03; let s4 = 0x14;
        let s5 = 0x0A; let s6 = 0x0; let s7 = 1; let s8 = 1;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(s_command).unwrap();

        let sw = &sim.sine_waves[0]; // SW #1 is at index 0
        assert_eq!(sw.enabled, true);
        assert_eq!(sw.amplitude, 0x258);
        assert_eq!(sw.offset, 0x190);
        assert_eq!(sw.frequency_base, 0x03);
        assert_eq!(sw.duty_cycle, 0x14);
        assert_eq!(sw.reset_value, 0x0A);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn e_command_updates_system_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // Exx<delay=01F4><step_en=01><retries=05><auto_reset=01><temp_err=01><seq_en=1><clk_err=1><i_err=1><v_err=1>
        let e_command = "<Exx01F4010501011111>";

        let s1 = 1; let s2 = 1; let s3 = 1; let s4 = 1;
        let s5 = 0x01; let s6 = 0x01; let s7 = 0x05; let s8 = 0x01; let s9 = 0x01F4;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8 + s9;

        sim.process_command(e_command).unwrap();

        let config = &sim.system_config;
        assert_eq!(config.stop_on_v_error, true);
        assert_eq!(config.stop_on_i_error, true);
        assert_eq!(config.stop_on_clk_error, true);
        assert_eq!(config.psu_sequence_enabled, true);
        assert_eq!(config.stop_on_temp_error, true);
        assert_eq!(config.auto_reset, true);
        assert_eq!(config.auto_reset_retries, 5);
        assert_eq!(config.psu_step_enabled, true);
        assert_eq!(config.psu_step_delay, 500);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn a_command_updates_system_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // Axx<s3=1><s2=064><s1=00C8><s4=00><s6=1><s5=000A><padding=00>
        let a_command = "<Axx106400C80001000A00>";

        let s1 = 0x00C8; // cal_temp
        let s2 = 0x064;  // offset
        let s3 = 1;      // pos_neg
        let _s4 = 0x00;   // Unused field from command string
        let s5 = 0x000A; // pwr_up_delay
        let s6 = 1;      // set_pt_enabled
        let s7 = 0x0A;   // Buggy re-parse of last two digits of s5
        // NOTE: The C code bug does NOT include s4 in the checksum but DOES include s7.
        let expected_checksum = s1 + s2 + s3 + s5 + s6 + s7;

        sim.process_command(a_command).unwrap();

        let config = &sim.system_config;
        assert_eq!(config.power_up_delay, 10);
        assert_eq!(config.set_point_enabled, true);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn f_command_updates_clock_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // Fxx<s9=1><s8=1><s7=00><s6=0A><s5=0><s4=CD><s3=AB><s2=FF><s1=FF>
        let f_command = "<Fxx11000A0CDABFFFF>";

        let expected_checksum = "11000A0CDABFFFF".chars().fold(0, |acc, c| acc + c.to_digit(16).unwrap());

        sim.process_command(f_command).unwrap();

        let config = &sim.system_config;
        assert_eq!(config.clocks_required, true);
        assert_eq!(config.clocks_restart_required, true);
        assert_eq!(config.clocks_restart_time, 600); // 10 * 60
        assert_eq!(config.clk32_mon_filter, !0xFFFF);
        assert_eq!(config.clk64_mon_filter, !0xCDAB);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn j_command_updates_sequence_delays() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command("<C1F5002>").unwrap();

        // Jxx<s1=1><s2=0><s3=64><s4=64><s5=00><s6=00><s7=64><s8=64>
        let j_command = "<Jxx10646400006464>";

        let s1 = 1; let s2 = 0; let s3 = 0x64; let s4 = 0x64;
        let s5 = 0x00; let s6 = 0x00; let s7 = 0x64; let s8 = 0x64;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(j_command).unwrap();

        let config = &sim.system_config;
        assert_eq!(config.sigs_mod_sequence_on, 1);
        assert_eq!(config.sigs_mod_sequence_off, 0);
        assert_eq!(config.seq_off_delay_3, 100);
        assert_eq!(config.seq_on_delay_3, 100);
        assert_eq!(config.seq_off_delay_2, 0);
        assert_eq!(config.seq_on_delay_2, 0);
        assert_eq!(config.seq_off_delay_1, 100);
        assert_eq!(config.seq_on_delay_1, 100);

        let end_response = sim.process_command("<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }
}
