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
    // Micro-stepping configuration
    pub ustep_steps: u32,
    pub ustep_delay: u32,
}

// Represents the state of an FPGA, including its pattern memory.
#[derive(Debug, Clone)]
pub struct Fpga {
    pub present: bool,
    // Using a Vec<u32> to represent pattern memory words.
    pub pattern_memory_a: Vec<u32>,
    pub pattern_memory_b: Vec<u32>,
    pub tristate_memory_a: Vec<u32>,
    pub tristate_memory_b: Vec<u32>,
}

impl Default for Fpga {
    fn default() -> Self {
        Self {
            present: false,
            // Pre-allocate memory to avoid resizing during data loading.
            // 0x100000 corresponds to 1M addresses.
            pattern_memory_a: vec![0; 0x100000],
            pattern_memory_b: vec![0; 0x100000],
            tristate_memory_a: vec![0; 0x100000],
            tristate_memory_b: vec![0; 0x100000],
        }
    }
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

// Represents the Power Temperature Cycling (PTC) configuration.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PtcConfig {
    pub enabled: bool,
    pub on_time_seconds: u32,
    pub off_time_seconds: u32,
}

// Represents the configuration for a single AMON/DUTMON test.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct AmonTest {
    pub test_type: u32,
    pub tp1_mux_ch: u32,
    pub tp1_amon_mux_a: u32,
    pub tp1_amon_mux_b: u32,
    pub tp2_mux_ch: u32,
    pub tp2_amon_mux_a: u32,
    pub tp2_amon_mux_b: u32,
    pub psu_link: u32,
    pub tp1_gain: f32,
    pub tp2_gain: f32,
    pub sum_gain: f32,
    // Fields for 'B' command
    pub tp1_peak_detect: u32,
    pub tp2_peak_detect: u32,
    pub tp1_samples: u32,
    pub tp2_samples: u32,
    pub board: u32,
    pub tp1_discharge: u32,
    pub tp2_discharge: u32,
    pub tag: u32,
    pub tp1_common_mux: u32,
    pub tp2_common_mux: u32,
    pub tp1_discharge_time: u32,
    pub tp2_discharge_time: u32,
    pub unit_type: u32,
}

// Represents the configuration for a single pattern loop.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct PatternLoop {
    pub start_address: u32,
    pub end_address: u32,
    pub count: u32,
}

// Represents the main pattern clock configuration.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct MainClockConfig {
    pub freq_low_byte: u32,
    pub freq_high_byte: u32,
    pub period_low_byte: u32,
    pub period_high_byte: u32,
    pub source: u32,
}

// Represents the Fractional Clock (FRC) configuration.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FrcConfig {
    pub frequency_1_4: u32,
    pub frequency_5_8: u32,
    pub period_1_4: u32,
    pub period_5_8: u32,
    pub source_1_4: u32,
    pub source_5_8: u32,
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
    // Power Temperature Cycling configuration
    pub ptc_config: PtcConfig,
    // AMON/DUTMON test configurations
    pub amon_tests: Vec<AmonTest>,
    pub amon_test_count: u32,
    // Micro-stepping global enable flag
    pub ustep_enabled: bool,
    // Pattern Loop configuration
    pub pattern_loops: [PatternLoop; 8],
    // Main pattern clock configuration
    pub main_clock_config: MainClockConfig,
    pub loop_enables: u32,
    pub repeat_count_1: u32,
    pub repeat_count_2: u32,
    // Fractional Clock configuration
    pub frc_config: FrcConfig,
    // Output routing configuration
    pub output_routing: [u32; 16],
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
            ptc_config: Default::default(),
            amon_tests: vec![AmonTest::default(); 100], // Pre-allocate for 100 tests
            amon_test_count: 0,
            ustep_enabled: false,
            pattern_loops: Default::default(),
            main_clock_config: Default::default(),
            loop_enables: 0,
            repeat_count_1: 0,
            repeat_count_2: 0,
            frc_config: Default::default(),
            output_routing: [0; 16],
            sram_address: 1,
            pattern_data_checksum: 0,
            driver_data_checksum: 0,
            is_pattern_data_loading: false,
            is_driver_data_loading: false,
        }
    }

    /// Parses the content of a command string into a `Command` enum.
    /// This is only used for 'C' commands which are known to be ASCII.
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

    /// Processes a command byte slice and returns the appropriate response.
    pub fn process_command(&mut self, command_bytes: &[u8]) -> Result<Option<String>, CommandError> {
        let start_byte = command_bytes.iter().position(|&b| b == b'<');
        let end_byte = command_bytes.iter().rposition(|&b| b == b'>');

        let content_bytes = match (start_byte, end_byte) {
            (Some(start), Some(end)) if end > start => &command_bytes[start + 1..end],
            _ => return Err(CommandError::InvalidFrame),
        };

        if content_bytes.is_empty() {
            return Err(CommandError::TooShort);
        }

        // Handle data loading commands first if a session is active.
        if self.is_pattern_data_loading {
            match content_bytes[0] {
                b'P' => {
                    self.handle_p_command(content_bytes)?;
                    return Ok(None);
                }
                b'R' => {
                    self.handle_r_command(content_bytes)?;
                    return Ok(None);
                }
                _ => {}
            }
        }

        if self.is_driver_data_loading {
            match content_bytes[0] {
                b'V' => { self.handle_v_command(content_bytes)?; return Ok(None); }
                b'Q' => { self.handle_q_command(content_bytes)?; return Ok(None); }
                b'T' => { self.handle_t_command(content_bytes)?; return Ok(None); }
                b'D' => { self.handle_d_command(content_bytes)?; return Ok(None); }
                b'S' => { self.handle_s_command(content_bytes)?; return Ok(None); }
                b'E' => { self.handle_e_command(content_bytes)?; return Ok(None); }
                b'A' => { self.handle_a_command(content_bytes)?; return Ok(None); }
                b'F' => { self.handle_f_command(content_bytes)?; return Ok(None); }
                b'J' => { self.handle_j_command(content_bytes)?; return Ok(None); }
                b'L' => { self.handle_l_command(content_bytes)?; return Ok(None); }
                b'X' => { self.handle_x_command(content_bytes)?; return Ok(None); }
                b'N' => { self.handle_n_command(content_bytes)?; return Ok(None); }
                b'G' => { self.handle_g_command(content_bytes)?; return Ok(None); }
                b'H' => { self.handle_h_command(content_bytes)?; return Ok(None); }
                b'K' => { self.handle_k_command(content_bytes)?; return Ok(None); }
                b'O' => { self.handle_o_command(content_bytes)?; return Ok(None); }
                b'M' => { self.handle_m_command(content_bytes)?; return Ok(None); }
                b'Z' => { self.handle_z_command(content_bytes)?; return Ok(None); }
                b'W' => { self.handle_w_command(content_bytes)?; return Ok(None); }
                b'U' => { self.handle_u_command(content_bytes)?; return Ok(None); }
                b'B' => { self.handle_b_command(content_bytes)?; return Ok(None); }
                _ => {} // Fall through to 'C' command check
            }
        }

        // Handle 'C' type control commands
        if content_bytes[0] == b'C' {
            // Control commands are always ASCII, so we can convert to &str for parsing.
            let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
            if content.len() < 5 {
                return Err(CommandError::TooShort);
            }

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
    fn handle_v_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_q_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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

    /// Parses an 'M' command, updates PSU uStep config, and updates the checksum.
    fn handle_m_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 20 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram6_psu_num = parse_hex(3, 5)? as usize;
        let sram5_steps = parse_hex(5, 8)?;
        let sram4_enable = parse_hex(8, 9)?;
        let sram3_delay = parse_hex(9, 13)?;
        let sram2 = parse_hex(13, 16)?; // Unused for state
        let sram1 = parse_hex(16, 19)?; // Unused for state
        // SRAM7 at index 19 is parsed in C but not used in checksum.

        self.ustep_enabled = sram4_enable == 1;

        if sram6_psu_num > 0 && sram6_psu_num <= self.psus.len() {
            let psu = &mut self.psus[sram6_psu_num - 1];
            psu.ustep_steps = sram5_steps;
            psu.ustep_delay = sram3_delay;
        }

        self.driver_data_checksum += sram1 + sram2 + sram3_delay + sram4_enable + sram5_steps + sram6_psu_num as u32;
        Ok(())
    }

    /// Parses a 'Z' command, updates PTC config, and updates the checksum.
    fn handle_z_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 15 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram1_enabled = parse_hex(3, 5)?;
        let sram2_on_time = parse_hex(5, 9)?;
        let sram3_off_time = parse_hex(9, 13)?;
        let sram4_unit_type = parse_hex(13, 15)?;

        self.ptc_config.enabled = sram1_enabled == 1;

        if sram4_unit_type == 1 { // Time is in seconds
            self.ptc_config.on_time_seconds = sram2_on_time;
            self.ptc_config.off_time_seconds = sram3_off_time;
        } else { // Time is in minutes (default)
            self.ptc_config.on_time_seconds = sram2_on_time * 60;
            self.ptc_config.off_time_seconds = sram3_off_time * 60;
        }

        self.driver_data_checksum += sram1_enabled + sram2_on_time + sram3_off_time + sram4_unit_type;
        Ok(())
    }

    /// Parses a 'W' command, updates AMON test config, and updates the checksum.
    fn handle_w_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 21 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram8_test_num = parse_hex(3, 5)? as usize;
        let sram7_type = parse_hex(5, 7)?;
        let sram6_tp1_mux = parse_hex(7, 9)?;
        let sram5_tp1_amon_a = parse_hex(9, 11)?;
        let sram4_tp1_amon_b = parse_hex(11, 13)?;
        let sram3_tp2_mux = parse_hex(13, 15)?;
        let sram2_tp2_amon_a = parse_hex(15, 17)?;
        let sram1_tp2_amon_b = parse_hex(17, 19)?;
        let sram9_psu_link = parse_hex(19, 21)?;

        if sram8_test_num > 0 && sram8_test_num <= self.amon_tests.len() {
            let test = &mut self.amon_tests[sram8_test_num - 1];
            test.test_type = sram7_type;
            test.tp1_mux_ch = sram6_tp1_mux;
            test.tp1_amon_mux_a = sram5_tp1_amon_a;
            test.tp1_amon_mux_b = sram4_tp1_amon_b;
            test.tp2_mux_ch = sram3_tp2_mux;
            test.tp2_amon_mux_a = sram2_tp2_amon_a;
            test.tp2_amon_mux_b = sram1_tp2_amon_b;
            test.psu_link = sram9_psu_link;
        }

        self.driver_data_checksum += sram1_tp2_amon_b + sram2_tp2_amon_a + sram3_tp2_mux + sram4_tp1_amon_b + sram5_tp1_amon_a + sram6_tp1_mux + sram7_type + sram8_test_num as u32 + sram9_psu_link;
        Ok(())
    }

    /// Parses a 'U' command, updates AMON gain config, and updates the checksum.
    fn handle_u_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram8_test_num = parse_hex(3, 5)? as usize;
        let sram4_test_count = parse_hex(17, 19)?;
        let sram3_sum_gain = parse_hex(13, 17)?;
        let sram2_tp2_gain = parse_hex(9, 13)?;
        let sram1_tp1_gain = parse_hex(5, 9)?;

        self.amon_test_count = sram4_test_count;

        if sram8_test_num > 0 && sram8_test_num <= self.amon_tests.len() {
            let test = &mut self.amon_tests[sram8_test_num - 1];
            test.tp1_gain = sram1_tp1_gain as f32 / 1000.0;
            test.tp2_gain = sram2_tp2_gain as f32 / 1000.0;
            test.sum_gain = sram3_sum_gain as f32 / 1000.0;
        }

        self.driver_data_checksum += sram1_tp1_gain + sram2_tp2_gain + sram3_sum_gain + sram4_test_count + sram8_test_num as u32;
        Ok(())
    }

    /// Parses a 'B' command, updates detailed AMON test config, and updates the checksum.
    fn handle_b_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 18 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let cmd_type = parse_hex(3, 4)?;
        let test_num = parse_hex(4, 6)? as usize;

        if test_num == 0 || test_num > self.amon_tests.len() {
            return Err(CommandError::InvalidParameter);
        }
        let test = &mut self.amon_tests[test_num - 1];
        self.amon_test_count = test_num as u32;

        let sram1 = parse_hex(8, 10)?;
        let sram2 = parse_hex(10, 12)?;
        let sram3 = parse_hex(12, 14)?;
        let sram4 = parse_hex(14, 16)?;
        let sram5 = parse_hex(16, 18)?;

        match cmd_type {
            1 => {
                test.tp1_mux_ch = sram1;
                test.tp1_peak_detect = sram2;
                test.tp2_mux_ch = sram3;
                test.tp2_peak_detect = sram4;
                test.test_type = sram5;
            }
            2 => {
                test.tp1_amon_mux_a = sram1;
                test.tp1_samples = sram2;
                test.tp2_amon_mux_a = sram3;
                test.tp2_samples = sram4;
                test.board = sram5;
            }
            3 => {
                test.tp1_amon_mux_b = sram1;
                test.tp1_discharge = sram2;
                test.tp2_amon_mux_b = sram3;
                test.tp2_discharge = sram4;
                test.tag = sram5;
            }
            4 => {
                test.tp1_common_mux = sram1;
                test.tp1_discharge_time = sram2;
                test.tp2_common_mux = sram3;
                test.tp2_discharge_time = sram4;
                test.unit_type = sram5;
            }
            _ => return Err(CommandError::InvalidParameter),
        }

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + test_num as u32 + cmd_type;
        Ok(())
    }

    /// Parses a 'T' command, updates timer state, and updates the checksum.
    fn handle_t_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_d_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_s_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_e_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_a_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_f_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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
    fn handle_j_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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

    /// Parses an 'L' command, updates pattern loop state, and updates the checksum.
    fn handle_l_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 11 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        // This handles the older, shorter variant of the 'L' command.
        let sram1_loop_num = parse_hex(3, 5)? as usize;
        let sram4_count = parse_hex(5, 7)?;
        let sram3_end_addr = parse_hex(7, 9)?;
        let sram2_start_addr = parse_hex(9, 11)?;

        if sram1_loop_num > 0 && sram1_loop_num <= self.pattern_loops.len() {
            let p_loop = &mut self.pattern_loops[sram1_loop_num - 1];
            p_loop.count = sram4_count;
            p_loop.end_address = sram3_end_addr;
            p_loop.start_address = sram2_start_addr;
        }

        self.driver_data_checksum += sram1_loop_num as u32 + sram2_start_addr + sram3_end_addr + sram4_count;
        Ok(())
    }

    /// Parses an 'X' command, updates clock and loop config, and updates the checksum.
    fn handle_x_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 14 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram1 = parse_hex(3, 5)?;
        let sram2 = parse_hex(5, 7)?;
        let sram3 = parse_hex(7, 9)?;
        let sram4 = parse_hex(9, 11)?;
        let sram5 = parse_hex(11, 12)?;
        let sram6 = parse_hex(12, 14)?;

        self.main_clock_config.freq_low_byte = sram1;
        self.main_clock_config.freq_high_byte = sram2;
        self.main_clock_config.period_low_byte = sram3;
        self.main_clock_config.period_high_byte = sram4;
        self.main_clock_config.source = sram5;
        self.loop_enables = sram6;

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6;
        Ok(())
    }

    /// Parses an 'N' command, updates loop repeat counts, and updates the checksum.
    fn handle_n_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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

        // Reconstruct the 32-bit values in little-endian order, matching the C code.
        self.repeat_count_1 = u32::from_le_bytes([sram1 as u8, sram2 as u8, sram3 as u8, sram4 as u8]);
        self.repeat_count_2 = u32::from_le_bytes([sram5 as u8, sram6 as u8, sram7 as u8, sram8 as u8]);

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8;
        Ok(())
    }

    /// Parses a 'G' command, updates FRC frequencies, and updates the checksum.
    fn handle_g_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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

        self.frc_config.frequency_1_4 = u32::from_le_bytes([sram1 as u8, sram2 as u8, sram3 as u8, sram4 as u8]);
        self.frc_config.frequency_5_8 = u32::from_le_bytes([sram5 as u8, sram6 as u8, sram7 as u8, sram8 as u8]);

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8;
        Ok(())
    }

    /// Parses an 'H' command, updates FRC periods, and updates the checksum.
    fn handle_h_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
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

        self.frc_config.period_1_4 = u32::from_le_bytes([sram1 as u8, sram2 as u8, sram3 as u8, sram4 as u8]);
        self.frc_config.period_5_8 = u32::from_le_bytes([sram5 as u8, sram6 as u8, sram7 as u8, sram8 as u8]);

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8;
        Ok(())
    }

    /// Parses a 'K' command, updates FRC sources, and updates the checksum.
    fn handle_k_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 11 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram8 = parse_hex(3, 4)?;
        let sram7 = parse_hex(4, 5)?;
        let sram6 = parse_hex(5, 6)?;
        let sram5 = parse_hex(6, 7)?;
        let sram4 = parse_hex(7, 8)?;
        let sram3 = parse_hex(8, 9)?;
        let sram2 = parse_hex(9, 10)?;
        let sram1 = parse_hex(10, 11)?;

        self.frc_config.source_1_4 = u32::from_le_bytes([sram1 as u8, sram2 as u8, sram3 as u8, sram4 as u8]);
        self.frc_config.source_5_8 = u32::from_le_bytes([sram5 as u8, sram6 as u8, sram7 as u8, sram8 as u8]);

        self.driver_data_checksum += sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8;
        Ok(())
    }

    /// Parses an 'O' command, updates output routing, and updates the checksum.
    fn handle_o_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 13 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram1_group = parse_hex(3, 5)? as usize;
        let sram2 = parse_hex(5, 7)?;
        let sram3 = parse_hex(7, 9)?;
        let sram4 = parse_hex(9, 11)?;
        let sram5 = parse_hex(11, 13)?;

        if sram1_group > 0 && sram1_group <= self.output_routing.len() {
            let routing_value = u32::from_le_bytes([sram2 as u8, sram3 as u8, sram4 as u8, sram5 as u8]);
            self.output_routing[sram1_group - 1] = routing_value;
        }

        self.driver_data_checksum += sram1_group as u32 + sram2 + sram3 + sram4 + sram5;
        Ok(())
    }

    /// Parses a 'P' command, updates FPGA memory, and updates the checksum.
    fn handle_p_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let bytes = content_bytes;
        let mut checksum_update: u32 = 0;

        if self.fpgas[1].present { // Two FPGAs
            if bytes.len() < 19 { return Err(CommandError::TooShort); }
            let sram1 = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
            let sram2 = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
            let sram3 = bytes[9] as u32;
            let sram4 = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
            let sram5 = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
            let sram6 = bytes[18] as u32;

            self.fpgas[0].pattern_memory_a[self.sram_address as usize] = sram1;
            self.fpgas[1].pattern_memory_a[self.sram_address as usize] = sram2;
            self.sram_address += 1;
            self.fpgas[0].pattern_memory_a[self.sram_address as usize] = sram4;
            self.fpgas[1].pattern_memory_a[self.sram_address as usize] = sram5;
            self.sram_address += 1;

            checksum_update += sram3 + sram6;
            for &byte in &bytes[1..9] { checksum_update += byte as u32; }
            for &byte in &bytes[10..18] { checksum_update += byte as u32; }
        } else { // One FPGA
            if bytes.len() < 21 { return Err(CommandError::TooShort); }
            let sram1 = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
            let sram2 = bytes[5] as u32;
            let sram3 = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
            let sram4 = bytes[10] as u32;
            let sram5 = u32::from_le_bytes(bytes[11..15].try_into().unwrap());
            let sram6 = bytes[15] as u32;
            let sram7 = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
            let sram8 = bytes[20] as u32;

            self.fpgas[0].pattern_memory_a[self.sram_address as usize] = sram1; self.sram_address += 1;
            self.fpgas[0].pattern_memory_a[self.sram_address as usize] = sram3; self.sram_address += 1;
            self.fpgas[0].pattern_memory_a[self.sram_address as usize] = sram5; self.sram_address += 1;
            self.fpgas[0].pattern_memory_a[self.sram_address as usize] = sram7; self.sram_address += 1;

            checksum_update += sram2 + sram4 + sram6 + sram8;
            for &byte in &bytes[1..5] { checksum_update += byte as u32; }
            for &byte in &bytes[6..10] { checksum_update += byte as u32; }
            for &byte in &bytes[11..15] { checksum_update += byte as u32; }
            for &byte in &bytes[16..20] { checksum_update += byte as u32; }
        }

        self.pattern_data_checksum += checksum_update;
        Ok(())
    }

    /// Parses an 'R' command, updates FPGA tristate memory, and updates the checksum.
    fn handle_r_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let bytes = content_bytes;
        let mut checksum_update: u32 = 0;

        if self.fpgas[1].present { // Two FPGAs
            if bytes.len() < 19 { return Err(CommandError::TooShort); }
            let sram1 = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
            let sram2 = u32::from_le_bytes(bytes[5..9].try_into().unwrap());
            let sram3 = bytes[9] as u32;
            let sram4 = u32::from_le_bytes(bytes[10..14].try_into().unwrap());
            let sram5 = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
            let sram6 = bytes[18] as u32;

            // Note the bitwise NOT, as seen in the C code.
            self.fpgas[0].tristate_memory_a[self.sram_address as usize] = !sram1;
            self.fpgas[1].tristate_memory_a[self.sram_address as usize] = !sram2;
            self.sram_address += 1;
            self.fpgas[0].tristate_memory_a[self.sram_address as usize] = !sram4;
            self.fpgas[1].tristate_memory_a[self.sram_address as usize] = !sram5;
            self.sram_address += 1;

            checksum_update += sram3 + sram6;
            for &byte in &bytes[1..9] { checksum_update += byte as u32; }
            for &byte in &bytes[10..18] { checksum_update += byte as u32; }
        } else { // One FPGA
            if bytes.len() < 21 { return Err(CommandError::TooShort); }
            let sram1 = u32::from_le_bytes(bytes[1..5].try_into().unwrap());
            let sram2 = bytes[5] as u32;
            let sram3 = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
            let sram4 = bytes[10] as u32;
            let sram5 = u32::from_le_bytes(bytes[11..15].try_into().unwrap());
            let sram6 = bytes[15] as u32;
            let sram7 = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
            let sram8 = bytes[20] as u32;

            // Note the bitwise NOT.
            self.fpgas[0].tristate_memory_a[self.sram_address as usize] = !sram1; self.sram_address += 1;
            self.fpgas[0].tristate_memory_a[self.sram_address as usize] = !sram3; self.sram_address += 1;
            self.fpgas[0].tristate_memory_a[self.sram_address as usize] = !sram5; self.sram_address += 1;
            self.fpgas[0].tristate_memory_a[self.sram_address as usize] = !sram7; self.sram_address += 1;

            checksum_update += sram2 + sram4 + sram6 + sram8;
            for &byte in &bytes[1..5] { checksum_update += byte as u32; }
            for &byte in &bytes[6..10] { checksum_update += byte as u32; }
            for &byte in &bytes[11..15] { checksum_update += byte as u32; }
            for &byte in &bytes[16..20] { checksum_update += byte as u32; }
        }

        self.pattern_data_checksum += checksum_update;
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
        let response = sim.process_command(b"<C1F03>").unwrap();
        assert_eq!(response, Some(String::from("#ON#")));
    }

    #[test]
    fn process_command_with_trailing_characters() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command(b"<C1F03>>>garbage").unwrap();
        assert_eq!(response, Some(String::from("#ON#")));
    }

    #[test]
    fn process_command_with_leading_characters() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command(b"noise<C1F03>").unwrap();
        assert_eq!(response, Some(String::from("#ON#")));
    }

    #[test]
    fn ignore_command_for_other_address() {
        let mut sim = Simulator::new(0x1F);
        let response = sim.process_command(b"<C2A03>").unwrap();
        assert_eq!(response, None);
    }

    #[test]
    fn reject_malformed_frame() {
        let mut sim = Simulator::new(0x1F);
        assert_eq!(sim.process_command(b"C1F03>").unwrap_err(), CommandError::InvalidFrame);
        assert_eq!(sim.process_command(b"<C1F03").unwrap_err(), CommandError::InvalidFrame);
        assert_eq!(sim.process_command(b">C1F03<").unwrap_err(), CommandError::InvalidFrame);
    }

    #[test]
    fn reject_too_short_command() {
        let mut sim = Simulator::new(0x1F);
        assert_eq!(sim.process_command(b"<C1F>").unwrap_err(), CommandError::TooShort);
    }

    #[test]
    fn reject_invalid_hex_address() {
        let mut sim = Simulator::new(0x1F);
        let result = sim.process_command(b"<CZZ03>");
        assert!(matches!(result, Err(CommandError::InvalidAddress(_))));
    }

    // --- Tests for specific command logic ---

    #[test]
    fn process_command_50_pattern_load_cycle() {
        let mut sim = Simulator::new(0x1F);
        let response1 = sim.process_command(b"<C1F5000>").unwrap();
        assert_eq!(response1, Some(String::from("#OK#")));
        let response2 = sim.process_command(b"<C1F5001>").unwrap();
        assert_eq!(response2, Some(String::from("#0,1,#")));
    }

    #[test]
    fn process_command_50_driver_load_cycle() {
        let mut sim = Simulator::new(0x1F);
        let response1 = sim.process_command(b"<C1F5002>").unwrap();
        assert_eq!(response1, Some(String::from("#OK#")));
        let response2 = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(response2, Some(String::from("#0#")));
    }

    #[test]
    fn process_sequence_on_off_commands() {
        let mut sim = Simulator::new(0x1F);
        let response_on = sim.process_command(b"<C1F03>").unwrap();
        assert_eq!(response_on, Some(String::from("#ON#")));
        let response_off = sim.process_command(b"<C1F04>").unwrap();
        assert_eq!(response_off, Some(String::from("#OFF#")));
    }

    #[test]
    fn checksum_validation_during_driver_load() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();
        let v_command = b"<Vxx0605004003002001>";
        let expected_checksum = 0x06 + 0x05 + 0x004 + 0x003 + 0x002 + 0x001;
        sim.process_command(v_command).unwrap();
        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn q_command_updates_psu_state_and_checksum() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();
        let q_command = b"<Qxx0306420C8007D0FA00>";
        let psu_num = 0x03;
        let delay = 0x064;
        let seq_id = 0x2;
        let cal_v = 0x0C80;
        let low_v = 0x07D;
        let high_v = 0x0FA;
        let expected_checksum = psu_num + delay + seq_id + cal_v + low_v + high_v;
        sim.process_command(q_command).unwrap();
        let psu = &sim.psus[2];
        assert_eq!(psu.sequence_id, 2);
        assert_eq!(psu.sequence_delay, 100);
        assert_eq!(psu.high_voltage_limit, 25.0);
        assert_eq!(psu.low_voltage_limit, 12.5);
        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn t_command_updates_timer_and_checksum() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();
        let t_command = b"<Txx0807060504030201>";
        let s1 = 0x01;
        let s2 = 0x02;
        let s3 = 0x03;
        let s4 = 0x04;
        let s5 = 0x05;
        let s6 = 0x06;
        let s7 = 0x07;
        let s8 = 0x08;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;
        sim.process_command(t_command).unwrap();
        assert_eq!(sim.timer_values, [s1, s2, s3, s4]);
        assert_eq!(sim.alarm_values, [s5, s6, s7, s8]);
        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn d_command_updates_psu_current_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();
        let d_command = b"<Dxx043E80C8006411>";
        let psu_num = 0x04;
        let i_cal = 0x3E80;
        let i_mon = 0xC80;
        let i_cal_off = 0x0641;
        let pos_neg = 1;
        let expected_checksum = psu_num + i_cal + i_mon + i_cal_off + pos_neg;
        sim.process_command(d_command).unwrap();
        let psu = &sim.psus[3];
        assert_eq!(psu.current_monitor_limit, 32.0);
        assert_eq!(psu.i_cal_val, 16.0);
        assert_eq!(psu.i_cal_offset_val, -16.01);
        assert_eq!(psu.pos_neg_i, 1);
        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn d_command_updates_psu_voltage_offset() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();
        let d_command = b"<Dxx07000000000320>";
        let psu_num = 0x07;
        let i_cal = 0x0;
        let i_mon = 0x0;
        let v_cal_off = 0x0032;
        let pos_neg = 0;
        let expected_checksum = psu_num + i_cal + i_mon + v_cal_off + pos_neg;
        sim.process_command(d_command).unwrap();
        let psu = &sim.psus[0];
        assert_eq!(psu.v_cal_offset_val, 0.5);
        assert_eq!(psu.pos_neg_v, 0);
        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn s_command_updates_sine_wave_state() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // S<sw_num=01><used=1><type=0><reset=0A><duty=14><freq=03><offset=190><amp=258>
        let s_command = b"<Sxx01100A1403190258>";

        let s1 = 0x258;
        let s2 = 0x190;
        let s3 = 0x03;
        let s4 = 0x14;
        let s5 = 0x0A;
        let s6 = 0x0;
        let s7 = 1;
        let s8 = 1;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(s_command).unwrap();

        let sw = &sim.sine_waves[0]; // SW #1 is at index 0
        assert_eq!(sw.enabled, true);
        assert_eq!(sw.amplitude, 0x258);
        assert_eq!(sw.offset, 0x190);
        assert_eq!(sw.frequency_base, 0x03);
        assert_eq!(sw.duty_cycle, 0x14);
        assert_eq!(sw.reset_value, 0x0A);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn e_command_updates_system_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Exx<delay=01F4><step_en=01><retries=05><auto_reset=01><temp_err=01><seq_en=1><clk_err=1><i_err=1><v_err=1>
        let e_command = b"<Exx01F4010501011111>";

        let s1 = 1;
        let s2 = 1;
        let s3 = 1;
        let s4 = 1;
        let s5 = 0x01;
        let s6 = 0x01;
        let s7 = 0x05;
        let s8 = 0x01;
        let s9 = 0x01F4;
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

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn a_command_updates_system_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Axx<s3=1><s2=064><s1=00C8><s4=00><s6=1><s5=000A><padding=00>
        let a_command = b"<Axx106400C80001000A00>";

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

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn f_command_updates_clock_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Fxx<s9=1><s8=1><s7=00><s6=0A><s5=0><s4=CD><s3=AB><s2=FF><s1=FF>
        let f_command = b"<Fxx11000A0CDABFFFF>";

        let expected_checksum = "11000A0CDABFFFF".chars().fold(0, |acc, c| acc + c.to_digit(16).unwrap());

        sim.process_command(f_command).unwrap();

        let config = &sim.system_config;
        assert_eq!(config.clocks_required, true);
        assert_eq!(config.clocks_restart_required, true);
        assert_eq!(config.clocks_restart_time, 600); // 10 * 60
        assert_eq!(config.clk32_mon_filter, !0xFFFF);
        assert_eq!(config.clk64_mon_filter, !0xCDAB);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn j_command_updates_sequence_delays() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Jxx<s1=1><s2=0><s3=64><s4=64><s5=00><s6=00><s7=64><s8=64>
        let j_command = b"<Jxx10646400006464>";

        let s1 = 1;
        let s2 = 0;
        let s3 = 0x64;
        let s4 = 0x64;
        let s5 = 0x00;
        let s6 = 0x00;
        let s7 = 0x64;
        let s8 = 0x64;
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

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn l_command_updates_loop_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Lxx<loop=01><count=0A><end=FF><start=00>
        let l_command = b"<Lxx010AFF00>";

        let s1 = 0x01; // loop num
        let s2 = 0x00; // start
        let s3 = 0xFF; // end
        let s4 = 0x0A; // count
        let expected_checksum = s1 + s2 + s3 + s4;

        sim.process_command(l_command).unwrap();

        let p_loop = &sim.pattern_loops[0]; // Loop #1 is at index 0
        assert_eq!(p_loop.start_address, 0x00);
        assert_eq!(p_loop.end_address, 0xFF);
        assert_eq!(p_loop.count, 0x0A);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn x_command_updates_clock_and_loop_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Xxx<f_low=28><f_high=00><p_low=14><p_high=00><src=0><loops=0F>
        let x_command = b"<Xxx2800140000F>";

        let s1 = 0x28; // f_low
        let s2 = 0x00; // f_high
        let s3 = 0x14; // p_low
        let s4 = 0x00; // p_high
        let s5 = 0;    // source
        let s6 = 0x0F; // loop_enables
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6;

        sim.process_command(x_command).unwrap();

        let clock = &sim.main_clock_config;
        assert_eq!(clock.freq_low_byte, 0x28);
        assert_eq!(clock.period_low_byte, 0x14);
        assert_eq!(clock.source, 0);
        assert_eq!(sim.loop_enables, 0x0F);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn n_command_updates_repeat_counts() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Nxx<s8=01><s7=02><s6=03><s5=04><s4=05><s3=06><s2=07><s1=08>
        let n_command = b"<Nxx0102030405060708>";

        let s1 = 0x08;
        let s2 = 0x07;
        let s3 = 0x06;
        let s4 = 0x05;
        let s5 = 0x04;
        let s6 = 0x03;
        let s7 = 0x02;
        let s8 = 0x01;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(n_command).unwrap();

        assert_eq!(sim.repeat_count_1, 0x05060708);
        assert_eq!(sim.repeat_count_2, 0x01020304);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn g_command_updates_frc_frequency() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Gxx<s8=01><s7=02><s6=03><s5=04><s4=05><s3=06><s2=07><s1=08>
        let g_command = b"<Gxx0102030405060708>";

        let s1 = 0x08;
        let s2 = 0x07;
        let s3 = 0x06;
        let s4 = 0x05;
        let s5 = 0x04;
        let s6 = 0x03;
        let s7 = 0x02;
        let s8 = 0x01;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(g_command).unwrap();

        assert_eq!(sim.frc_config.frequency_1_4, 0x05060708);
        assert_eq!(sim.frc_config.frequency_5_8, 0x01020304);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn h_command_updates_frc_period() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Hxx<s8=11><s7=22><s6=33><s5=44><s4=55><s3=66><s2=77><s1=88>
        let h_command = b"<Hxx1122334455667788>";

        let s1 = 0x88;
        let s2 = 0x77;
        let s3 = 0x66;
        let s4 = 0x55;
        let s5 = 0x44;
        let s6 = 0x33;
        let s7 = 0x22;
        let s8 = 0x11;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(h_command).unwrap();

        assert_eq!(sim.frc_config.period_1_4, 0x55667788);
        assert_eq!(sim.frc_config.period_5_8, 0x11223344);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn k_command_updates_frc_source() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Kxx<s8=1><s7=2><s6=3><s5=4><s4=5><s3=6><s2=7><s1=8>
        let k_command = b"<Kxx12345678>";

        let s1 = 8;
        let s2 = 7;
        let s3 = 6;
        let s4 = 5;
        let s5 = 4;
        let s6 = 3;
        let s7 = 2;
        let s8 = 1;
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8;

        sim.process_command(k_command).unwrap();

        assert_eq!(sim.frc_config.source_1_4, 0x05060708);
        assert_eq!(sim.frc_config.source_5_8, 0x01020304);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn o_command_updates_output_routing() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();

        // Oxx<group=09><s2=01><s3=02><s4=03><s5=04>
        let o_command = b"<Oxx0901020304>";

        let s1 = 0x09;
        let s2 = 0x01;
        let s3 = 0x02;
        let s4 = 0x03;
        let s5 = 0x04;
        let expected_checksum = s1 + s2 + s3 + s4 + s5;

        sim.process_command(o_command).unwrap();

        assert_eq!(sim.output_routing[8], 0x04030201); // Group 9 is index 8

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn p_command_loads_data_one_fpga() {
        let mut sim = Simulator::new(0x1F);
        sim.fpgas[1].present = false; // Ensure single FPGA mode
        sim.process_command(b"<C1F5000>").unwrap(); // Start pattern loading

        // P<data1><\ctrl1><data2><\ctrl2><data3><\ctrl3><data4><\ctrl4>
        let p_command = b"<P\x01\x02\x03\x04\x11\x05\x06\x07\x08\x22\x09\x0A\x0B\x0C\x33\x0D\x0E\x0F\x10\x44>";

        let data1 = 0x04030201;
        let ctrl1 = 0x11;
        let data2 = 0x08070605;
        let ctrl2 = 0x22;
        let data3 = 0x0C0B0A09;
        let ctrl3 = 0x33;
        let data4 = 0x100F0E0D;
        let ctrl4 = 0x44;

        let checksum = (ctrl1 + ctrl2 + ctrl3 + ctrl4) +
            (0x01 + 0x02 + 0x03 + 0x04) + (0x05 + 0x06 + 0x07 + 0x08) +
            (0x09 + 0x0A + 0x0B + 0x0C) + (0x0D + 0x0E + 0x0F + 0x10);

        sim.process_command(p_command).unwrap();

        assert_eq!(sim.fpgas[0].pattern_memory_a[1], data1);
        assert_eq!(sim.fpgas[0].pattern_memory_a[2], data2);
        assert_eq!(sim.fpgas[0].pattern_memory_a[3], data3);
        assert_eq!(sim.fpgas[0].pattern_memory_a[4], data4);
        assert_eq!(sim.sram_address, 5);

        let end_response = sim.process_command(b"<C1F5001>").unwrap();
        assert_eq!(end_response, Some(format!("#{},5,#", checksum)));
    }

    #[test]
    fn p_command_loads_data_two_fpgas() {
        let mut sim = Simulator::new(0x1F);
        sim.fpgas[1].present = true; // Ensure dual FPGA mode
        sim.process_command(b"<C1F5000>").unwrap(); // Start pattern loading

        // P<data1a><data1b><\ctrl1><data2a><data2b><\ctrl2>
        let p_command = b"<P\x01\x02\x03\x04\x11\x12\x13\x14\xAA\x05\x06\x07\x08\x15\x16\x17\x18\xBB>";

        let data1a = 0x04030201;
        let data1b = 0x14131211;
        let ctrl1 = 0xAA;
        let data2a = 0x08070605;
        let data2b = 0x18171615;
        let ctrl2 = 0xBB;

        let checksum = (ctrl1 + ctrl2) +
            (0x01 + 0x02 + 0x03 + 0x04 + 0x11 + 0x12 + 0x13 + 0x14) +
            (0x05 + 0x06 + 0x07 + 0x08 + 0x15 + 0x16 + 0x17 + 0x18);

        sim.process_command(p_command).unwrap();

        assert_eq!(sim.fpgas[0].pattern_memory_a[1], data1a);
        assert_eq!(sim.fpgas[1].pattern_memory_a[1], data1b);
        assert_eq!(sim.fpgas[0].pattern_memory_a[2], data2a);
        assert_eq!(sim.fpgas[1].pattern_memory_a[2], data2b);
        assert_eq!(sim.sram_address, 3);

        let end_response = sim.process_command(b"<C1F5001>").unwrap();
        assert_eq!(end_response, Some(format!("#{},3,#", checksum)));
    }

    #[test]
    fn m_command_updates_ustep_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Mxx<psu=02><steps=064><enable=1><delay=00C8><s2=000><s1=000><s7=0>
        let m_command = b"<Mxx02064100C80000000>";

        let psu_num = 0x02;
        let steps = 0x064;
        let enable = 1;
        let delay = 0x00C8;
        let s2 = 0;
        let s1 = 0;
        let expected_checksum = psu_num + steps + enable + delay + s2 + s1;

        sim.process_command(m_command).unwrap();

        assert_eq!(sim.ustep_enabled, true);
        let psu = &sim.psus[1]; // PSU #2 is at index 1
        assert_eq!(psu.ustep_steps, 100);
        assert_eq!(psu.ustep_delay, 200);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn z_command_updates_ptc_config_minutes() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Zxx<enabled=01><on_time=000A><off_time=001E><unit_type=00>
        let z_command = b"<Zxx01000A001E00>";

        let s1 = 0x01; // enabled
        let s2 = 0x0A; // on_time (10 mins)
        let s3 = 0x1E; // off_time (30 mins)
        let s4 = 0x00; // unit_type (minutes)
        let expected_checksum = s1 + s2 + s3 + s4;

        sim.process_command(z_command).unwrap();

        assert_eq!(sim.ptc_config.enabled, true);
        assert_eq!(sim.ptc_config.on_time_seconds, 10 * 60);
        assert_eq!(sim.ptc_config.off_time_seconds, 30 * 60);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn z_command_updates_ptc_config_seconds() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Zxx<enabled=01><on_time=003C><off_time=00B4><unit_type=01>
        let z_command = b"<Zxx01003C00B401>";

        let s1 = 0x01; // enabled
        let s2 = 0x3C; // on_time (60s)
        let s3 = 0xB4; // off_time (180s)
        let s4 = 0x01; // unit_type (seconds)
        let expected_checksum = s1 + s2 + s3 + s4;

        sim.process_command(z_command).unwrap();

        assert_eq!(sim.ptc_config.enabled, true);
        assert_eq!(sim.ptc_config.on_time_seconds, 60);
        assert_eq!(sim.ptc_config.off_time_seconds, 180);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn w_command_updates_amon_test_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Wxx<test=01><type=02><tp1_mux=03><tp1_amon_a=04><tp1_amon_b=05><tp2_mux=06><tp2_amon_a=07><tp2_amon_b=08><psu_link=09>
        let w_command = b"<Wxx010203040506070809>";

        let s8 = 0x01; // test num
        let s7 = 0x02; // type
        let s6 = 0x03; // tp1 mux
        let s5 = 0x04; // tp1 amon a
        let s4 = 0x05; // tp1 amon b
        let s3 = 0x06; // tp2 mux
        let s2 = 0x07; // tp2 amon a
        let s1 = 0x08; // tp2 amon b
        let s9 = 0x09; // psu link
        let expected_checksum = s1 + s2 + s3 + s4 + s5 + s6 + s7 + s8 + s9;

        sim.process_command(w_command).unwrap();

        let test = &sim.amon_tests[0]; // Test #1 is at index 0
        assert_eq!(test.test_type, s7);
        assert_eq!(test.tp1_mux_ch, s6);
        assert_eq!(test.tp1_amon_mux_a, s5);
        assert_eq!(test.tp1_amon_mux_b, s4);
        assert_eq!(test.tp2_mux_ch, s3);
        assert_eq!(test.tp2_amon_mux_a, s2);
        assert_eq!(test.tp2_amon_mux_b, s1);
        assert_eq!(test.psu_link, s9);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn u_command_updates_amon_gain_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Uxx<test=01><tp1_gain=03E8><tp2_gain=07D0><sum_gain=0BB8><count=0A>
        let u_command = b"<Uxx0103E807D00BB80A>";

        let s8 = 0x01;   // test_num
        let s1 = 0x03E8; // tp1_gain (1000 -> 1.0)
        let s2 = 0x07D0; // tp2_gain (2000 -> 2.0)
        let s3 = 0x0BB8; // sum_gain (3000 -> 3.0)
        let s4 = 0x0A;   // test_count
        let expected_checksum = s1 + s2 + s3 + s4 + s8;

        sim.process_command(u_command).unwrap();

        assert_eq!(sim.amon_test_count, 10);
        let test = &sim.amon_tests[0]; // Test #1 is at index 0
        assert_eq!(test.tp1_gain, 1.0);
        assert_eq!(test.tp2_gain, 2.0);
        assert_eq!(test.sum_gain, 3.0);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn b_command_updates_amon_config() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Type 1: Mux and Test Type
        let b_command1 = b"<Bxx101000A0B0C0D01>";
        sim.process_command(b_command1).unwrap();
        let test1 = &sim.amon_tests[0];
        assert_eq!(test1.tp1_mux_ch, 0x0A);
        assert_eq!(test1.tp1_peak_detect, 0x0B);
        assert_eq!(test1.tp2_mux_ch, 0x0C);
        assert_eq!(test1.tp2_peak_detect, 0x0D);
        assert_eq!(test1.test_type, 0x01);

        // Type 2: AMON Mux A and Samples
        let b_command2 = b"<Bxx2020014321E6405>";
        sim.process_command(b_command2).unwrap();
        let test2 = &sim.amon_tests[1];
        assert_eq!(test2.tp1_amon_mux_a, 0x14);
        assert_eq!(test2.tp1_samples, 0x32);
        assert_eq!(test2.tp2_amon_mux_a, 0x1E);
        assert_eq!(test2.tp2_samples, 0x64);
        assert_eq!(test2.board, 0x05);

        // Type 3: AMON Mux B and Discharge
        let b_command3 = b"<Bxx30300010203040F>";
        sim.process_command(b_command3).unwrap();
        let test3 = &sim.amon_tests[2];
        assert_eq!(test3.tp1_amon_mux_b, 0x01);
        assert_eq!(test3.tp1_discharge, 0x02);
        assert_eq!(test3.tp2_amon_mux_b, 0x03);
        assert_eq!(test3.tp2_discharge, 0x04);
        assert_eq!(test3.tag, 0x0F);

        // Type 4: Common Mux and Discharge Time
        let b_command4 = b"<Bxx40400196421C80A>";
        sim.process_command(b_command4).unwrap();
        let test4 = &sim.amon_tests[3];
        assert_eq!(test4.tp1_common_mux, 0x19);
        assert_eq!(test4.tp1_discharge_time, 0x64);
        assert_eq!(test4.tp2_common_mux, 0x21);
        assert_eq!(test4.tp2_discharge_time, 0xC8);
        assert_eq!(test4.unit_type, 0x0A);

        let end_response = sim.process_command(b"<C1F5003>").unwrap();
        let expected_checksum = (0x01 + 0x01 + 0x0A + 0x0B + 0x0C + 0x0D + 0x01) +
            (0x02 + 0x02 + 0x14 + 0x32 + 0x1E + 0x64 + 0x05) +
            (0x03 + 0x03 + 0x01 + 0x02 + 0x03 + 0x04 + 0x0F) +
            (0x04 + 0x04 + 0x19 + 0x64 + 0x21 + 0xC8 + 0x0A);
        assert_eq!(end_response, Some(format!("#{}#", expected_checksum)));
    }
}
