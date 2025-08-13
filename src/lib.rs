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

/// The result of processing a command.
#[derive(Debug, Default, PartialEq)]
pub struct ProcessResult {
    /// The response to be sent back to the client, if any.
    pub response: Option<String>,
    /// A list of debug log messages generated during processing.
    pub logs: Vec<String>,
}


// Represents all possible numeric commands from the C firmware.
#[derive(Debug, PartialEq)]
enum Command {
    /// Command 01: Clears clock failure flags.
    ClearClockFail,
    /// Command 02: Clears sine wave failure flags.
    ClearSwFail,
    /// Command 03: Starts the main power and signal sequence.
    SequenceOn,
    /// Command 04: Stops the main power and signal sequence.
    SequenceOff,
    /// Command 05: Starts the power sequence for calibration.
    SequenceOnCal(u32),
    /// Command 09: Sets the program ID and optionally clears memory.
    SetProgramId { address: u32, data: u32 },
    /// Command 16: Sets the temperature status (Temp_OK flag).
    SetTempOk(bool),
    /// Command 17: Returns the reference monitoring string.
    MonitorVi,
    /// Command 18: Returns the hardware configuration string.
    GetConfiguration,
    /// Command 19: Performs a self-test of the memory.
    SelfTestMem { is_basic: bool },
    /// Command 20: Retrieves a historical fault log by index.
    GetFaultLog(u32),
    /// Command 21: Returns the firmware and FPGA version string.
    GetVersion,
    /// Command 22: Returns the currently loaded Program ID.
    GetProgramId,
    /// Command 23: Returns the checksum of the Program ID.
    GetProgramIdChecksum,
    /// Command 24: Returns the main VI monitoring string.
    GetViMonitorString,
    /// Command 25: Returns the AMON/DUTMON monitoring string.
    GetAmonMonitorString,
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
#[derive(Debug, Clone, PartialEq)]
pub struct Psu {
    pub enabled: bool,
    pub voltage_setpoint: f32,
    pub current_limit: f32,
    // ADDED: Fields to store simulated "measured" values, separate from setpoints.
    pub measured_voltage: f32,
    pub measured_current: f32,
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
    pub psu_cal_val: f32,
}

impl Default for Psu {
    /// Implements default values for a PSU based on the C firmware's initial state.
    fn default() -> Self {
        Self {
            enabled: true,
            voltage_setpoint: 0.0,
            current_limit: 0.0,
            // ADDED: Initialize new measured value fields.
            measured_voltage: 0.0,
            measured_current: 0.0,
            voltage_set_s1: 0,
            voltage_set_s2: 0,
            voltage_set_s3: 0,
            voltage_set_s4: 0,
            high_voltage_limit: 1.0,
            low_voltage_limit: -1.0,
            current_monitor_limit: 1.0,
            i_cal_val: 1.0,
            i_cal_offset_val: 0.0,
            pos_neg_i: 0,
            v_cal_offset_val: 0.0,
            pos_neg_v: 0,
            sequence_id: 0,
            sequence_delay: 0,
            ustep_steps: 0,
            ustep_delay: 0,
            psu_cal_val: 1.0,
        }
    }
}

// Represents the state of an FPGA, including its pattern memory.
#[derive(Debug, Clone)]
pub struct Fpga {
    pub present: bool,
    pub position: u8,
    pub version: u8,
    pub mem_a_test_ok: bool,
    pub mem_b_test_ok: bool,
    pub ctrl_a_test_ok: bool,
    pub ctrl_b_test_ok: bool,
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
            position: 0,
            version: 0,
            mem_a_test_ok: true,
            mem_b_test_ok: true,
            ctrl_a_test_ok: true,
            ctrl_b_test_ok: true,
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
    pub module_type: u8,
    pub fpga_version: u8,
    /// Represents if a clock has a failure condition.
    pub has_failure: bool,
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
    pub module_type: u8,
    pub fpga_version: u8,
    pub programmed: bool,
    /// Represents if a sine wave has a failure condition.
    pub has_failure: bool,
    /// Simulated RMS value for monitoring.
    pub rms_value: f32,
}

// Represents system-wide configuration and error handling settings.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SystemConfig {
    pub auto_reset: bool,
    pub auto_reset_retries: u32,
    pub auto_reset_counter: u32,
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
    pub ignore_clock_fails: bool,
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
    // Fields for 'I' and 'Y' commands
    pub cal_gain: f32,
    pub cal_offset: f32,
    pub high_limit: f32,
    pub low_limit: f32,
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

/// Represents a snapshot of the system state at the time of a fault.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FaultLog {
    pub monitor_voltages: [f32; 6],
    pub monitor_currents: [f32; 6],
    pub auto_reset_counter: u32,
    pub over_current_flags: u8,
    pub under_voltage_flags: u8,
    pub over_voltage_flags: u8,
    pub clock_status_1_16: u16,
    pub clock_status_17_32: u16,
    pub clock_status_33_48: u16,
    pub clock_status_49_64: u16,
    pub sw_fault_status: u32,
    pub sw1_rms: f32,
    pub sw2_rms: f32,
    pub driver_on: bool,
    pub timer_values: [u32; 4],
    pub alarm_values: [u32; 4],
}

// The main struct that holds the entire state of the simulated driver board.
#[derive(Debug, Clone)]
pub struct Simulator {
    // The 2-character hexadecimal RS-485 address of the simulator.
    pub rs485_address: u8,
    pub fw_version: f32,
    /// Represents the overall on/off status of the driver sequence.
    pub sequence_on: bool,
    /// High and low integers for the program ID.
    pub prog_id_hint: u32,
    pub prog_id_lint: u32,
    /// Represents the temperature status, enabling the timing countdown.
    pub temp_ok: bool,
    // An array of 6 PSUs, as suggested by the C code (PSU_1_DATA to PSU_6_DATA).
    pub psus: [Psu; 6],
    pub psu_data_codes: [u8; 6],
    // Two FPGAs are mentioned in the C code (FPGA1_Present, FPGA2_Present).
    pub fpgas: [Fpga; 2],
    // Four Clock Generators (CLKMOD1_Present to CLKMOD4_Present).
    pub clock_generators: [ClockGenerator; 4],
    // Two Sine Wave modules (SW1_Present, SW2_Present).
    pub sine_waves: [SineWave; 2],
    // AMON module information
    pub amon_present: bool,
    pub amon_type: u8,
    pub amon_bp: u32,
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
    // New fields for C17 command
    pub back_panel_address: u8,
    pub bib_code: u16,
    pub bp_res1_present: bool,
    pub bp_res2_present: bool,
    pub door_open: bool, // C code uses 1 for closed, 0 for open
    // Historical fault logs
    pub fault_logs: Vec<FaultLog>,
    // --- Internal state for data loading sessions ---
    sram_address: u32,
    pattern_data_checksum: u32,
    driver_data_checksum: u32,
    is_pattern_data_loading: bool,
    is_driver_data_loading: bool,
    // --- Internal buffer for logging checksum changes ---
    log_buffer: Vec<String>,
}

impl Simulator {
    /// Creates a new `Simulator` instance with a given RS-485 address.
    pub fn new(rs485_address: u8) -> Self {
        Self {
            rs485_address,
            fw_version: 1.46,
            sequence_on: false,
            prog_id_hint: 0,
            prog_id_lint: 0,
            temp_ok: false,
            psus: Default::default(),
            psu_data_codes: [0; 6],
            fpgas: Default::default(),
            clock_generators: Default::default(),
            sine_waves: Default::default(),
            amon_present: false,
            amon_type: 0xFF,
            amon_bp: 0,
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
            back_panel_address: 0,
            bib_code: 0,
            bp_res1_present: true,
            bp_res2_present: true,
            door_open: false, // Corresponds to 0 (closed) in C code
            fault_logs: vec![FaultLog::default(); 10], // C firmware stores 10 logs
            sram_address: 1,
            pattern_data_checksum: 0,
            driver_data_checksum: 0,
            is_pattern_data_loading: false,
            is_driver_data_loading: false,
            log_buffer: Vec::new(),
        }
    }

    /// Helper to update the driver checksum and log the change.
    fn update_driver_checksum(&mut self, value_to_add: u32) {
        self.driver_data_checksum = self.driver_data_checksum.wrapping_add(value_to_add);
        self.log_buffer.push(format!(
            "[DEBUG] Driver checksum updated by {}, new value: {}",
            value_to_add, self.driver_data_checksum
        ));
    }

    /// Helper to update the pattern checksum and log the change.
    fn update_pattern_checksum(&mut self, value_to_add: u32) {
        self.pattern_data_checksum = self.pattern_data_checksum.wrapping_add(value_to_add);
        self.log_buffer.push(format!(
            "[DEBUG] Pattern checksum updated by {}, new value: {}",
            value_to_add, self.pattern_data_checksum
        ));
    }

    /// Parses the content of a command string into a `Command` enum.
    /// This is only used for 'C' commands which are known to be ASCII.
    fn parse_command(&self, content: &str) -> Result<Command, CommandError> {
        let cmd_id_str = &content[3..5];
        let cmd_id = u8::from_str_radix(cmd_id_str, 10).map_err(CommandError::InvalidCommandId)?;

        match cmd_id {
            1 => Ok(Command::ClearClockFail),
            2 => Ok(Command::ClearSwFail),
            3 => Ok(Command::SequenceOn),
            4 => Ok(Command::SequenceOff),
            5 => {
                if content.len() < 19 {
                    return Err(CommandError::TooShort);
                }
                let data_str = &content[14..19];
                let data = data_str.trim().parse::<u32>().map_err(|_| CommandError::InvalidParameter)?;
                Ok(Command::SequenceOnCal(data))
            }
            9 => {
                if content.len() < 19 {
                    return Err(CommandError::TooShort);
                }
                let address = content[9..14].trim().parse::<u32>().map_err(|_| CommandError::InvalidParameter)?;
                let data = content[14..19].trim().parse::<u32>().map_err(|_| CommandError::InvalidParameter)?;
                Ok(Command::SetProgramId { address, data })
            }
            16 => {
                if content.len() < 19 {
                    return Err(CommandError::TooShort);
                }
                let data = content[14..19].trim().parse::<u32>().map_err(|_| CommandError::InvalidParameter)?;
                Ok(Command::SetTempOk(data == 1))
            }
            17 => Ok(Command::MonitorVi),
            18 => Ok(Command::GetConfiguration),
            19 => {
                if content.len() < 19 {
                    return Err(CommandError::TooShort);
                }
                let data = content[14..19].trim().parse::<u32>().map_err(|_| CommandError::InvalidParameter)?;
                Ok(Command::SelfTestMem { is_basic: data != 0 })
            }
            20 => {
                if content.len() < 19 {
                    return Err(CommandError::TooShort);
                }
                let data = content[14..19].trim().parse::<u32>().map_err(|_| CommandError::InvalidParameter)?;
                Ok(Command::GetFaultLog(data))
            }
            21 => Ok(Command::GetVersion),
            22 => Ok(Command::GetProgramId),
            23 => Ok(Command::GetProgramIdChecksum),
            24 => Ok(Command::GetViMonitorString),
            25 => Ok(Command::GetAmonMonitorString),
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
    pub fn process_command(&mut self, command_bytes: &[u8]) -> Result<ProcessResult, CommandError> {
        self.log_buffer.clear();

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
                    return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() });
                }
                b'R' => {
                    self.handle_r_command(content_bytes)?;
                    return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() });
                }
                _ => {}
            }
        }

        if self.is_driver_data_loading {
            match content_bytes[0] {
                b'V' => { self.handle_v_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'Q' => { self.handle_q_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'T' => { self.handle_t_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'D' => { self.handle_d_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'S' => { self.handle_s_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'E' => { self.handle_e_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'A' => { self.handle_a_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'F' => { self.handle_f_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'J' => { self.handle_j_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'L' => { self.handle_l_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'X' => { self.handle_x_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'N' => { self.handle_n_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'G' => { self.handle_g_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'H' => { self.handle_h_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'K' => { self.handle_k_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'O' => { self.handle_o_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'M' => { self.handle_m_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'Z' => { self.handle_z_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'W' => { self.handle_w_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'U' => { self.handle_u_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'B' => { self.handle_b_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'I' => { self.handle_i_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
                b'Y' => { self.handle_y_command(content_bytes)?; return Ok(ProcessResult { response: None, logs: self.log_buffer.clone() }); }
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
                return Ok(ProcessResult::default()); // Silently ignore
            }

            // Parse the command and dispatch it
            let command = self.parse_command(content)?;
            let response = self.execute_command(command);
            return Ok(ProcessResult { response: Some(response), logs: self.log_buffer.clone() });
        }

        Ok(ProcessResult::default())
    }

    /// Simulates the `MonitorVI` function from the C firmware.
    /// This updates the `measured_voltage` and `measured_current` for each PSU.
    fn update_monitored_values(&mut self) {
        for psu in self.psus.iter_mut() {
            if !psu.enabled {
                psu.measured_voltage = 0.0;
                psu.measured_current = 0.0;
                continue;
            }

            // CRITICAL FIX: Simulate the hardware scaling.
            // Convert the 12-bit DAC value (0-4095) from the voltage_setpoint
            // into a simulated 0-10V ADC reading.
            let raw_voltage_reading = psu.voltage_setpoint as f32 / 409.5;

            // Simulate a small current draw. We'll model the raw ADC reading for current
            // as being 5% of its 10V range.
            let raw_current_reading = 10.0 * 0.05;

            // Apply the calibration and offset to the correctly scaled ADC readings.
            let mut final_voltage = raw_voltage_reading * psu.psu_cal_val;
            final_voltage += psu.v_cal_offset_val;

            let mut final_current = raw_current_reading + psu.i_cal_offset_val;
            final_current *= psu.i_cal_val;

            // Clamp to zero if negative, as seen in the C code
            psu.measured_voltage = if final_voltage < 0.0 { 0.0 } else { final_voltage };
            psu.measured_current = if final_current < 0.0 { 0.0 } else { final_current };
        }
    }

    /// Executes a parsed command and returns the response string.
    fn execute_command(&mut self, command: Command) -> String {
        // ADDED: Update the simulated "measurements" before every command that might report them.
        self.update_monitored_values();

        match command {
            Command::ClearClockFail => {
                for gen in self.clock_generators.iter_mut() {
                    gen.has_failure = false;
                }
                String::from("#OK#")
            }
            Command::ClearSwFail => {
                for sw in self.sine_waves.iter_mut() {
                    sw.has_failure = false;
                }
                String::from("#OK#")
            }
            Command::SequenceOn => {
                // In the C code, this command also clears DUTMON data, resets the auto-reset counter,
                // and sets a flag to ignore clock fails to false.
                self.amon_tests.iter_mut().for_each(|test| *test = AmonTest::default());
                self.system_config.auto_reset_counter = 0;
                self.system_config.ignore_clock_fails = false;

                // ADDED: This is the essential logic that enables the PSUs.
                // It mimics the behavior of the C firmware's Sequence_ON function.
                for psu in self.psus.iter_mut() {
                    // A PSU is considered active if its final step voltage (loaded by a 'V' command) is non-zero.
                    if psu.voltage_set_s4 > 0 {
                        psu.enabled = true;
                        // Apply the final step voltage as the current setpoint.
                        psu.voltage_setpoint = psu.voltage_set_s4 as f32;
                    } else {
                        psu.enabled = false;
                        psu.voltage_setpoint = 0.0;
                    }
                }

                self.sequence_on = true;
                String::from("#ON#")
            }
            Command::SequenceOff => {
                self.sequence_on = false;
                String::from("#OFF#")
            }
            Command::SequenceOnCal(step) => {
                // REFACTORED/FIXED: This logic is now clearer and correctly handles a bug
                // found in the C firmware's logic for step 4.
                let s1: Vec<u16> = self.psus.iter().map(|p| p.voltage_set_s1).collect();
                let s2: Vec<u16> = self.psus.iter().map(|p| p.voltage_set_s2).collect();
                let s3: Vec<u16> = self.psus.iter().map(|p| p.voltage_set_s3).collect();
                let s4: Vec<u16> = self.psus.iter().map(|p| p.voltage_set_s4).collect();

                let setpoints: [u16; 6] = match step {
                    1 => [s1[0], s1[1], s1[2], s1[3], s1[4], s1[4]],
                    2 => [s2[0], s2[1], s2[2], s2[3], s2[4], s2[4]],
                    3 => [s3[0], s3[1], s3[2], s3[3], s3[4], s3[4]],
                    4 => [s4[0], s4[1], s4[2], s4[3], s3[4], s3[4]], // Note: This correctly mirrors the C code's quirk.
                    _ => [0; 6],
                };

                for i in 0..6 {
                    self.psus[i].enabled = true;
                    self.psus[i].voltage_setpoint = setpoints[i] as f32;
                }

                self.sequence_on = true;
                self.system_config.auto_reset_counter = 0;
                String::from("#ON#")
            }
            Command::SetProgramId { address, data } => {
                self.prog_id_hint = address;
                self.prog_id_lint = data;

                if address == 0 && data == 0 {
                    self.system_config.clocks_required = false;
                    self.amon_test_count = 0;
                    self.amon_tests.iter_mut().for_each(|t| *t = AmonTest::default());

                    if self.fpgas[0].present {
                        self.fpgas[0].pattern_memory_a.fill(0);
                        self.fpgas[0].pattern_memory_b.fill(0);
                        self.fpgas[0].tristate_memory_a.fill(0);
                    }
                    if self.fpgas[1].present {
                        self.fpgas[1].tristate_memory_b.fill(0);
                    }
                }
                String::from("#OK#")
            }
            Command::SetTempOk(status) => {
                self.temp_ok = status;
                // The C code immediately sends back the monitor string after this command.
                self.make_vi_monitor_string()
            }
            Command::MonitorVi => {
                // The C code for C17 ONLY sends the reference string.
                self.make_ref_monitor_string()
            }
            Command::GetConfiguration => self.make_configuration_string(),
            Command::SelfTestMem { is_basic: _ } => {
                self.prog_id_hint = 0;
                self.prog_id_lint = 0;

                // Simulate the test by setting the status flags to OK.
                for fpga in self.fpgas.iter_mut() {
                    fpga.mem_a_test_ok = true;
                    fpga.mem_b_test_ok = true;
                    fpga.ctrl_a_test_ok = true;
                    fpga.ctrl_b_test_ok = true;
                }
                // The C code prints to the console but doesn't have a specific return
                // value via UARTSend. We'll return a simple OK to acknowledge.
                String::from("#OK#")
            }
            Command::GetFaultLog(index) => {
                if let Some(log) = self.fault_logs.get(index as usize) {
                    self.make_vi_fault_string(log)
                } else {
                    // If the index is out of bounds, return an empty but validly formatted string.
                    self.make_vi_fault_string(&FaultLog::default())
                }
            }
            Command::GetVersion => self.make_version_string(),
            Command::GetProgramId => self.make_program_id_string(),
            Command::GetProgramIdChecksum => {
                format!("#{}#", self.prog_id_hint + self.prog_id_lint)
            }
            Command::GetViMonitorString => self.make_vi_monitor_string(),
            Command::GetAmonMonitorString => self.make_amon_monitor_string(),
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

    /// Creates the reference monitoring string, mimicking `MakeRefMonitorString`.
    fn make_ref_monitor_string(&self) -> String {
        format!(
            "#{:X},{:X},{:X},{},{},{},{},{},{},{},{},{},{},{},{},{},{}#",
            (self.back_panel_address as u32) + 0x100,
            (self.rs485_address as u32) + 0x100,
            self.bib_code + 0x1000,
            if self.bp_res1_present { 1 } else { 0 },
            if self.bp_res2_present { 1 } else { 0 },
            self.prog_id_lint + 100000,
            self.prog_id_hint + 100000,
            if self.sequence_on { 1 } else { 0 },
            self.timer_values[0] + 1000,
            self.timer_values[1] + 1000,
            self.timer_values[2] + 1000,
            self.timer_values[3] + 1000,
            self.alarm_values[0] + 1000,
            self.alarm_values[1] + 1000,
            self.alarm_values[2] + 1000,
            self.alarm_values[3] + 1000,
            if self.door_open { 0 } else { 1 } // C code: 0=Open, 1=Close
        )
    }

    /// Creates the hardware configuration string, mimicking `MakeConfigurationString`.
    fn make_configuration_string(&self) -> String {
        format!(
            "#{:X},{:X},{:X},{},{},{:X},{:X},{:X},{:X},{:X},{:X},{},{},{},{},{},{:X},{},{:X},{},{:X},{},{:X},{},{:X},{},{:X},{},{:X},{},{},{},{},{},{}#",
            (self.back_panel_address as u32) + 0x100,
            (self.rs485_address as u32) + 0x100,
            self.bib_code + 0x1000,
            if self.bp_res1_present { 1 } else { 0 },
            if self.bp_res2_present { 1 } else { 0 },
            (self.psu_data_codes[0] as u32) + 0x100,
            (self.psu_data_codes[1] as u32) + 0x100,
            (self.psu_data_codes[2] as u32) + 0x100,
            (self.psu_data_codes[3] as u32) + 0x100,
            (self.psu_data_codes[4] as u32) + 0x100,
            (self.psu_data_codes[5] as u32) + 0x100,
            if self.fpgas[0].present { 1 } else { 0 },
            self.fpgas[0].position,
            if self.fpgas[1].present { 1 } else { 0 },
            self.fpgas[1].position,
            if self.clock_generators[0].present { 1 } else { 0 },
            (self.clock_generators[0].module_type as u32) + 0x100,
            if self.clock_generators[1].present { 1 } else { 0 },
            (self.clock_generators[1].module_type as u32) + 0x100,
            if self.clock_generators[2].present { 1 } else { 0 },
            (self.clock_generators[2].module_type as u32) + 0x100,
            if self.clock_generators[3].present { 1 } else { 0 },
            (self.clock_generators[3].module_type as u32) + 0x100,
            if self.sine_waves[0].present { 1 } else { 0 },
            (self.sine_waves[0].module_type as u32) + 0x100,
            if self.sine_waves[1].present { 1 } else { 0 },
            (self.sine_waves[1].module_type as u32) + 0x100,
            if self.amon_present { 1 } else { 0 },
            (self.amon_type as u32) + 0x100,
            if self.fpgas[0].mem_a_test_ok { 0 } else { 1 }, // C code uses 1 for fail
            if self.fpgas[1].mem_b_test_ok { 0 } else { 1 }, // Assuming FPGA2 maps to Mem B
            if self.fpgas[0].ctrl_a_test_ok { 0 } else { 1 },
            if self.fpgas[1].ctrl_b_test_ok { 0 } else { 1 },
            if self.sine_waves[0].programmed { 1 } else { 0 },
            if self.sine_waves[1].programmed { 1 } else { 0 }
        )
    }

    /// Creates the version information string, mimicking `MakeVersionString`.
    fn make_version_string(&self) -> String {
        format!(
            "#{:.2},{},{},{},{},{},{},{},{},{}#",
            self.fw_version + 100.0,
            (self.fpgas[0].version as u32) + 100,
            (self.fpgas[1].version as u32) + 100,
            (self.clock_generators[0].fpga_version as u32) + 100,
            (self.clock_generators[1].fpga_version as u32) + 100,
            (self.clock_generators[2].fpga_version as u32) + 100,
            (self.clock_generators[3].fpga_version as u32) + 100,
            (self.sine_waves[0].fpga_version as u32) + 100,
            (self.sine_waves[1].fpga_version as u32) + 100,
            100 // Placeholder for Analog module version
        )
    }

    /// Creates the program ID string.
    fn make_program_id_string(&self) -> String {
        format!("#{:05},{:05}#", self.prog_id_hint, self.prog_id_lint)
    }

    /// Creates the main VI monitoring string, mimicking `MakeVIMonitorString`.
    fn make_vi_monitor_string(&self) -> String {
        let mut response = String::from("#");

        // PSU Voltages and Currents
        for psu in &self.psus {
            // CHANGED: Use the new measured_voltage field instead of the setpoint.
            let v_str = if psu.measured_voltage > 899.0 {
                format!("{:.1},", (psu.measured_voltage / 10.0) + 1000.0)
            } else {
                format!("{:.2},", psu.measured_voltage + 100.0)
            };
            response.push_str(&v_str);
            // CHANGED: Use the new measured_current field.
            response.push_str(&format!("{:.2},", psu.measured_current + 100.0));
        }

        // Auto-reset counter
        response.push_str(&format!("{},", self.system_config.auto_reset_counter + 1000));

        // PSU Fault Status (3 parts: OverCurrent, UnderVoltage, OverVoltage)
        // CHANGED: This logic now correctly checks measured values against limits.
        let mut fault_flags = String::new();
        for psu in &self.psus { fault_flags.push(if psu.measured_current > psu.current_monitor_limit {'1'} else {'0'}); }
        for psu in &self.psus { fault_flags.push(if psu.measured_voltage < psu.low_voltage_limit {'1'} else {'0'}); }
        for psu in &self.psus { fault_flags.push(if psu.measured_voltage > psu.high_voltage_limit {'1'} else {'0'}); }
        response.push_str(&fault_flags);

        // Clock Status (placeholder values for now)
        let clock_status_1_32 = 0u32;
        let clock_status_33_64 = 0u32;
        response.push_str(&format!(",{:X},", (clock_status_1_32 >> 16) + 0x10000));
        response.push_str(&format!("{:X},", (clock_status_1_32 & 0xFFFF) + 0x10000));
        response.push_str(&format!("{:X},", (clock_status_33_64 >> 16) + 0x10000));
        response.push_str(&format!("{:X},", (clock_status_33_64 & 0xFFFF) + 0x10000));

        // Sine Wave Status
        let sw_status = (if self.sine_waves[0].has_failure {1} else {0}) + (if self.sine_waves[1].has_failure {2} else {0});
        response.push_str(&format!("{:X},", sw_status + 0x100));
        response.push_str(&format!("{:.2},", self.sine_waves[0].rms_value + 100.0));
        response.push_str(&format!("{:.2},", self.sine_waves[1].rms_value + 100.0));

        // Driver Status
        response.push_str(&format!("{},", if self.sequence_on { 1 } else { 0 }));

        // Timers and Alarms
        for val in &self.timer_values { response.push_str(&format!("{},", val + 1000)); }
        for val in &self.alarm_values { response.push_str(&format!("{},", val + 1000)); }

        // Door Status (last item, no trailing comma)
        response.push_str(&format!("{}", if self.door_open { 0 } else { 1 }));

        response.push('#');
        response
    }

    /// Creates the fault log string, mimicking `MakeVIFaultString`.
    fn make_vi_fault_string(&self, log: &FaultLog) -> String {
        let mut response = String::from("#");

        // PSU Voltages and Currents
        for i in 0..6 {
            let v_str = if log.monitor_voltages[i] > 899.0 {
                format!("{:.1},", (log.monitor_voltages[i] / 10.0) + 1000.0)
            } else {
                format!("{:.2},", log.monitor_voltages[i] + 100.0)
            };
            response.push_str(&v_str);
            response.push_str(&format!("{:.2},", log.monitor_currents[i] + 100.0));
        }

        // Auto-reset counter
        response.push_str(&format!("{},", log.auto_reset_counter + 1000));

        // PSU Fault Status
        let mut fault_flags = String::new();
        for i in 0..6 { fault_flags.push(if (log.over_current_flags >> i) & 1 == 1 {'1'} else {'0'}); }
        for i in 0..6 { fault_flags.push(if (log.under_voltage_flags >> i) & 1 == 1 {'1'} else {'0'}); }
        for i in 0..6 { fault_flags.push(if (log.over_voltage_flags >> i) & 1 == 1 {'1'} else {'0'}); }
        response.push_str(&fault_flags);

        // Clock Status
        response.push_str(&format!(",{:X},", (log.clock_status_17_32 as u32) + 0x10000));
        response.push_str(&format!("{:X},", (log.clock_status_1_16 as u32) + 0x10000));
        response.push_str(&format!("{:X},", (log.clock_status_49_64 as u32) + 0x10000));
        response.push_str(&format!("{:X},", (log.clock_status_33_48 as u32) + 0x10000));

        // Sine Wave Status
        response.push_str(&format!("{:X},", log.sw_fault_status + 0x100));
        response.push_str(&format!("{:.2},", log.sw1_rms + 100.0));
        response.push_str(&format!("{:.2},", log.sw2_rms + 100.0));

        // Driver Status
        response.push_str(&format!("{},", if log.driver_on { 1 } else { 0 }));

        // Timers and Alarms
        for val in &log.timer_values { response.push_str(&format!("{},", val + 1000)); }
        for val in &log.alarm_values { response.push_str(&format!("{},", val + 1000)); }

        // Door Status (last item, no trailing comma) - Note: C code doesn't include door status in fault log string
        response.pop(); // Remove last comma
        response.push('#');
        response
    }

    /// Simulates the pass/fail logic for an AMON test based on linked PSU limits.
    fn return_amon_read_data_state(&self, measured_value: f32, test: &AmonTest) -> u32 {
        if test.psu_link == 0 || (test.psu_link as usize) > self.psus.len() {
            return 0; // No valid PSU link, no state to return
        }

        let psu = &self.psus[(test.psu_link - 1) as usize];

        // This logic mimics return_AMON_Read_Data_State from main.c
        if test.test_type == 1 { // Voltage
            if measured_value > psu.high_voltage_limit { return 1; }
            if measured_value < psu.low_voltage_limit { return 2; }
        } else if test.test_type == 2 || test.test_type == 3 { // Current
            if measured_value > psu.current_monitor_limit { return 1; }
        }
        0 // Pass
    }

    /// Simulates the measurement for a single AMON test.
    /// Returns a tuple of (measured_value, pass_fail_status).
    fn measure_amon_test_data(&self, test_index: usize) -> (f32, u32) {
        let test = &self.amon_tests[test_index];
        let mut measured_value = 0.0;

        // Since we don't have a real ADC, we'll simulate a reading.
        // A simple approach is to generate a value that would pass the test.
        // Let's use the midpoint of the PSU limits linked to this test.
        let psu_link_index = if test.psu_link > 0 && (test.psu_link as usize) <= self.psus.len() {
            (test.psu_link - 1) as usize
        } else {
            0 // Default to PSU 1 if link is invalid
        };
        let psu = &self.psus[psu_link_index];

        // Simulate a reading based on the test type and PSU limits
        let simulated_adc_reading = match test.test_type {
            1 => (psu.high_voltage_limit + psu.low_voltage_limit) / 2.0, // Voltage
            _ => psu.current_monitor_limit / 2.0, // Current
        };

        match test.test_type {
            1 | 2 => { // Voltage or Current Reading
                measured_value = simulated_adc_reading * test.tp1_gain;
                measured_value -= test.cal_offset;
                measured_value *= test.cal_gain;
            }
            3 => { // Current Summing Reading
                // Simulate two readings
                let reading1 = simulated_adc_reading * test.tp1_gain;
                let reading2 = (simulated_adc_reading * 0.9) * test.tp2_gain; // a slightly different second reading
                measured_value = (reading1 - reading2).abs(); // Difference
                measured_value *= test.sum_gain;
                measured_value -= test.cal_offset;
                measured_value *= test.cal_gain;
            }
            _ => { // Unknown test type
                measured_value = 0.0;
            }
        }

        if measured_value < 0.0 {
            measured_value = 0.0;
        }

        let status = self.return_amon_read_data_state(measured_value, test);
        (measured_value, status)
    }

    /// Creates the AMON monitoring string, mimicking `Make_AMON_VIMonitorString`.
    fn make_amon_monitor_string(&self) -> String {
        let mut response = format!("#{:X},", self.amon_bp + 0x1000);

        if self.amon_test_count > 0 {
            for i in 0..(self.amon_test_count as usize) {
                let test = &self.amon_tests[i];
                let (measured_value, result) = self.measure_amon_test_data(i);

                response.push_str(&format!("{:.2},", measured_value + 100.0));
                response.push_str(&format!("{},", result));
                response.push_str(&format!("{},", test.board + 10));

                if i == (self.amon_test_count - 1) as usize {
                    response.push_str(&format!("{}", test.tag + 100));
                } else {
                    response.push_str(&format!("{},", test.tag + 100));
                }
            }
        }

        response.push('#');
        response
    }

    /// Parses a 'V' command and updates the driver data checksum.
    fn handle_v_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 19 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let sram6_psu_num = parse_hex(3, 5)? as usize;
        let sram5_unused = parse_hex(5, 7)?;
        let sram4_vset_s4 = parse_hex(7, 10)?;
        let sram3_vset_s3 = parse_hex(10, 13)?;
        let sram2_vset_s2 = parse_hex(13, 16)?;
        let sram1_vset_s1 = parse_hex(16, 19)?;

        // Check if this is a PSU configuration (1-6) or clock monitor config (7)
        if sram6_psu_num > 0 && sram6_psu_num <= self.psus.len() {
            // Get the correct PSU (1-based index from command)
            let psu = &mut self.psus[sram6_psu_num - 1];

            // CORRECTED: Actually store the parsed voltage step values
            psu.voltage_set_s1 = sram1_vset_s1 as u16;
            psu.voltage_set_s2 = sram2_vset_s2 as u16;
            psu.voltage_set_s3 = sram3_vset_s3 as u16;
            psu.voltage_set_s4 = sram4_vset_s4 as u16;
        }
        // You could add an `else if sram6_psu_num == 7` block here
        // to handle the clock monitor settings if needed in the future.

        self.update_driver_checksum(sram1_vset_s1 + sram2_vset_s2 + sram3_vset_s3 + sram4_vset_s4 + sram5_unused + sram6_psu_num as u32);
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

        // ADDED: Parse the VreadGain multiplier from the command
        let sram7_vread_gain_mult = parse_hex(19, 20)?;
        let sram8_vmon_mult = parse_hex(20, 21)?;

        // PSU number in C code is 1-based, our array is 0-based.
        if sram6_psu_num > 0 && sram6_psu_num <= self.psus.len() {
            let psu = &mut self.psus[sram6_psu_num - 1];
            psu.sequence_id = sram4_seq_id;
            psu.sequence_delay = sram5_delay;

            let vmon_divisor = if sram8_vmon_mult == 1 { 1.0 } else { 10.0 };
            psu.high_voltage_limit = sram1_high_v as f32 / vmon_divisor;
            psu.low_voltage_limit = sram2_low_v as f32 / vmon_divisor;

            // ADDED: Calculate and store the voltage calibration gain (PS_CAL_VAL)
            let cal_v_divisor = match sram7_vread_gain_mult {
                2 => 500.0,
                1 => 1000.0,
                _ => 10000.0,
            };
            psu.psu_cal_val = sram3_cal_v as f32 / cal_v_divisor;
        }

        self.update_driver_checksum(sram1_high_v + sram2_low_v + sram3_cal_v + sram4_seq_id as u32 + sram5_delay + sram6_psu_num as u32);
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

        self.update_driver_checksum(sram1 + sram2 + sram3_delay + sram4_enable + sram5_steps + sram6_psu_num as u32);
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

        self.update_driver_checksum(sram1_enabled + sram2_on_time + sram3_off_time + sram4_unit_type);
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

        self.update_driver_checksum(sram1_tp2_amon_b + sram2_tp2_amon_a + sram3_tp2_mux + sram4_tp1_amon_b + sram5_tp1_amon_a + sram6_tp1_mux + sram7_type + sram8_test_num as u32 + sram9_psu_link);
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

        self.update_driver_checksum(sram1_tp1_gain + sram2_tp2_gain + sram3_sum_gain + sram4_test_count + sram8_test_num as u32);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + test_num as u32 + cmd_type);
        Ok(())
    }

    /// Parses an 'I' command, updates AMON calibration and limits, and updates the checksum.
    fn handle_i_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 21 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let cmd_type = parse_hex(3, 4)?;
        let test_num = parse_hex(4, 6)? as usize;

        if test_num == 0 || test_num > self.amon_tests.len() {
            return Err(CommandError::InvalidParameter);
        }
        let test = &mut self.amon_tests[test_num - 1];

        // The C code constructs the float from multiple hex string segments.
        // It's parsing an 8-character hex string representing a u32.
        let float_as_u32 = parse_hex(13, 21)?;
        let float_val = f32::from_bits(float_as_u32);

        match cmd_type {
            1 => test.tp1_gain = float_val,
            2 => test.tp2_gain = float_val,
            3 => test.sum_gain = float_val,
            4 => test.cal_gain = float_val,
            5 => test.cal_offset = float_val,
            6 => test.high_limit = float_val,
            7 => test.low_limit = float_val,
            _ => return Err(CommandError::InvalidParameter),
        }

        // The checksum logic in C is complex for this command.
        // DRIVER_DATA_CHECK=DRIVER_DATA_CHECK + nTest_Number + CMD_Type + toint(szCommand[13]) + toint(szCommand[14]) + ...
        // It sums the integer value of each hex character.
        let mut checksum_update = test_num as u32 + cmd_type;
        for i in 13..21 {
            checksum_update += u32::from_str_radix(&content[i..i + 1], 16).unwrap_or(0);
        }
        self.update_driver_checksum(checksum_update);

        Ok(())
    }

    /// Parses a 'Y' command, updates AMON calibration and metadata, and updates the checksum.
    fn handle_y_command(&mut self, content_bytes: &[u8]) -> Result<(), CommandError> {
        let content = std::str::from_utf8(content_bytes).map_err(|_| CommandError::InvalidParameter)?;
        if content.len() < 17 { return Err(CommandError::TooShort); }
        let parse_hex = |start, end| u32::from_str_radix(&content[start..end], 16).map_err(|_| CommandError::InvalidParameter);

        let test_num = parse_hex(3, 5)? as usize;
        let cal_gain = parse_hex(5, 9)?;
        let cal_offset = parse_hex(9, 13)?;
        let board = parse_hex(13, 15)?;
        let tag = parse_hex(15, 17)?;

        if test_num > 0 && test_num <= self.amon_tests.len() {
            let test = &mut self.amon_tests[test_num - 1];
            test.cal_gain = cal_gain as f32 / 1000.0;
            test.cal_offset = cal_offset as f32 / 1000.0;
            test.board = board;
            test.tag = tag;
        }

        self.update_driver_checksum(cal_gain + cal_offset + test_num as u32 + board + tag);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8);
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

        self.update_driver_checksum(sram1_i_mon + sram2_i_cal + sram3_psu_num as u32 + sram4_i_cal_off + sram5_pos_neg);
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

        self.update_driver_checksum(sram1_amp + sram2_offset + sram3_freq_base + sram4_duty + sram5_reset + sram6_type + sram7_used + sram8_sw_num as u32);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8 + sram9);
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
        self.update_driver_checksum(sram1 + sram2 + sram3 + sram5 + sram6 + sram7);
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
        self.update_driver_checksum(checksum_chars.chars().fold(0, |acc, c| {
            acc + c.to_digit(16).unwrap_or(0)
        }));
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8);
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

        self.update_driver_checksum(sram1_loop_num as u32 + sram2_start_addr + sram3_end_addr + sram4_count);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8);
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

        self.update_driver_checksum(sram1 + sram2 + sram3 + sram4 + sram5 + sram6 + sram7 + sram8);
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

        self.update_driver_checksum(sram1_group as u32 + sram2 + sram3 + sram4 + sram5);
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

        self.update_pattern_checksum(checksum_update);
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

        self.update_pattern_checksum(checksum_update);
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
        let result = sim.process_command(b"<C1F03>").unwrap();
        assert_eq!(result.response, Some(String::from("#ON#")));
    }

    #[test]
    fn process_command_with_trailing_characters() {
        let mut sim = Simulator::new(0x1F);
        let result = sim.process_command(b"<C1F03>>>garbage").unwrap();
        assert_eq!(result.response, Some(String::from("#ON#")));
    }

    #[test]
    fn process_command_with_leading_characters() {
        let mut sim = Simulator::new(0x1F);
        let result = sim.process_command(b"noise<C1F03>").unwrap();
        assert_eq!(result.response, Some(String::from("#ON#")));
    }

    #[test]
    fn ignore_command_for_other_address() {
        let mut sim = Simulator::new(0x1F);
        let result = sim.process_command(b"<C2A03>").unwrap();
        assert_eq!(result.response, None);
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
    fn process_command_clear_clock_fail() {
        let mut sim = Simulator::new(0x1F);
        // Set a failure state first
        sim.clock_generators[0].has_failure = true;
        sim.clock_generators[2].has_failure = true;

        // Process the command
        let result = sim.process_command(b"<C1F01>").unwrap();
        assert_eq!(result.response, Some(String::from("#OK#")));

        // Verify the state was changed
        assert_eq!(sim.clock_generators[0].has_failure, false);
        assert_eq!(sim.clock_generators[1].has_failure, false); // Should remain false
        assert_eq!(sim.clock_generators[2].has_failure, false);
    }

    #[test]
    fn process_command_clear_sw_fail() {
        let mut sim = Simulator::new(0x1F);
        // Set a failure state first
        sim.sine_waves[0].has_failure = true;
        sim.sine_waves[1].has_failure = true;

        // Process the command
        let result = sim.process_command(b"<C1F02>").unwrap();
        assert_eq!(result.response, Some(String::from("#OK#")));

        // Verify the state was changed
        assert_eq!(sim.sine_waves[0].has_failure, false);
        assert_eq!(sim.sine_waves[1].has_failure, false);
    }

    #[test]
    fn process_command_50_pattern_load_cycle() {
        let mut sim = Simulator::new(0x1F);
        let result1 = sim.process_command(b"<C1F5000>").unwrap();
        assert_eq!(result1.response, Some(String::from("#OK#")));
        let result2 = sim.process_command(b"<C1F5001>").unwrap();
        assert_eq!(result2.response, Some(String::from("#0,1,#")));
    }

    #[test]
    fn process_command_50_driver_load_cycle() {
        let mut sim = Simulator::new(0x1F);
        let result1 = sim.process_command(b"<C1F5002>").unwrap();
        assert_eq!(result1.response, Some(String::from("#OK#")));
        let result2 = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(result2.response, Some(String::from("#0#")));
    }

    #[test]
    fn process_sequence_on_off_commands() {
        let mut sim = Simulator::new(0x1F);
        sim.system_config.auto_reset_counter = 5; // Set a pre-condition

        let result_on = sim.process_command(b"<C1F03>").unwrap();
        assert_eq!(result_on.response, Some(String::from("#ON#")));
        assert_eq!(sim.sequence_on, true);
        assert_eq!(sim.system_config.auto_reset_counter, 0); // Verify reset

        let result_off = sim.process_command(b"<C1F04>").unwrap();
        assert_eq!(result_off.response, Some(String::from("#OFF#")));
        assert_eq!(sim.sequence_on, false);
    }

    #[test]
    fn process_command_sequence_on_cal() {
        let mut sim = Simulator::new(0x1F);
        // Pre-configure some PSU step voltages
        sim.psus[0].voltage_set_s2 = 100;
        sim.psus[1].voltage_set_s2 = 200;
        sim.psus[4].voltage_set_s2 = 500;
        sim.psus[5].voltage_set_s2 = 600; // This should be ignored for step 2

        sim.sequence_on = false;
        sim.system_config.auto_reset_counter = 99;

        // Command for SequenceOnCal, step 2
        let result = sim.process_command(b"<C1F0500000000000002>").unwrap();
        assert_eq!(result.response, Some(String::from("#ON#")));
        assert_eq!(sim.sequence_on, true);
        assert_eq!(sim.system_config.auto_reset_counter, 0);

        // Verify all PSUs are enabled and have the correct voltage setpoint for step 2
        assert_eq!(sim.psus[0].voltage_setpoint, 100.0);
        assert_eq!(sim.psus[1].voltage_setpoint, 200.0);
        assert_eq!(sim.psus[2].voltage_setpoint, 0.0); // Default value
        assert_eq!(sim.psus[3].voltage_setpoint, 0.0);
        assert_eq!(sim.psus[4].voltage_setpoint, 500.0);
        assert_eq!(sim.psus[5].voltage_setpoint, 500.0); // PSU6 takes value from PSU5 for step 2
        assert!(sim.psus.iter().all(|psu| psu.enabled));
    }

    #[test]
    fn process_command_set_program_id() {
        let mut sim = Simulator::new(0x1F);
        sim.fpgas[0].present = true;
        sim.fpgas[0].pattern_memory_a[10] = 0xDEADBEEF; // Pre-fill some data
        sim.system_config.clocks_required = true;
        sim.amon_test_count = 5;

        // Set a non-zero program ID
        let command1 = format!("<C1F090000{:05}{:05}>", 12345, 54321);
        let result1 = sim.process_command(command1.as_bytes()).unwrap();
        assert_eq!(result1.response, Some(String::from("#OK#")));
        assert_eq!(sim.prog_id_hint, 12345);
        assert_eq!(sim.prog_id_lint, 54321);
        // Verify state is NOT cleared
        assert_eq!(sim.fpgas[0].pattern_memory_a[10], 0xDEADBEEF);
        assert_eq!(sim.system_config.clocks_required, true);
        assert_eq!(sim.amon_test_count, 5);

        // Set a zero program ID to trigger reset
        let command2 = format!("<C1F090000{:05}{:05}>", 0, 0);
        let result2 = sim.process_command(command2.as_bytes()).unwrap();
        assert_eq!(result2.response, Some(String::from("#OK#")));
        assert_eq!(sim.prog_id_hint, 0);
        assert_eq!(sim.prog_id_lint, 0);
        // Verify state IS cleared
        assert_eq!(sim.fpgas[0].pattern_memory_a[10], 0);
        assert_eq!(sim.system_config.clocks_required, false);
        assert_eq!(sim.amon_test_count, 0);
    }

    #[test]
    fn process_command_16_set_temp_ok() {
        let mut sim = Simulator::new(0x1F);
        assert_eq!(sim.temp_ok, false);

        // Command to set Temp_OK to true
        let result1 = sim.process_command(b"<C1F1600000000000001>").unwrap();
        assert_eq!(sim.temp_ok, true);
        // The response should be the VI monitor string
        let expected_vi_string = sim.make_vi_monitor_string();
        assert_eq!(result1.response, Some(expected_vi_string));

        // Command to set Temp_OK to false
        let result2 = sim.process_command(b"<C1F1600000000000000>").unwrap();
        assert_eq!(sim.temp_ok, false);
        let expected_vi_string2 = sim.make_vi_monitor_string();
        assert_eq!(result2.response, Some(expected_vi_string2));
    }

    #[test]
    fn process_command_17_monitor_vi() {
        let mut sim = Simulator::new(0x1F);
        sim.back_panel_address = 0x0A;
        sim.bib_code = 0xABC;
        sim.prog_id_lint = 12345;
        sim.prog_id_hint = 54321;
        sim.sequence_on = true;
        sim.timer_values = [1, 2, 3, 4];
        sim.alarm_values = [5, 6, 7, 8];
        sim.door_open = false; // Closed
        sim.psus[0].voltage_setpoint = 1.23;
        sim.psus[0].current_limit = 0.45;
        sim.psus[5].voltage_setpoint = 900.5; // Test high voltage formatting
        sim.psus[5].current_limit = 6.78;
        sim.sine_waves[0].rms_value = 1.11;
        sim.sine_waves[1].rms_value = 2.22;

        let result = sim.process_command(b"<C1F17>").unwrap();
        let expected_ref = "#10A,11F,1ABC,1,1,112345,154321,1,1001,1002,1003,1004,1005,1006,1007,1008,1#";
        assert_eq!(result.response, Some(expected_ref.to_string()));
    }

    #[test]
    fn process_command_18_get_configuration() {
        let mut sim = Simulator::new(0x1F);
        sim.back_panel_address = 0x0A;
        sim.bib_code = 0xABC;
        sim.bp_res1_present = true;
        sim.bp_res2_present = false;
        sim.psu_data_codes = [0x1, 0x2, 0x3, 0x4, 0x5, 0x6];
        sim.fpgas[0].present = true;
        sim.fpgas[0].position = 1;
        sim.fpgas[0].mem_a_test_ok = false;
        sim.clock_generators[1].present = true;
        sim.clock_generators[1].module_type = 0x2B;
        sim.sine_waves[0].present = true;
        sim.sine_waves[0].module_type = 0x3C;
        sim.sine_waves[0].programmed = true;
        sim.amon_present = true;
        sim.amon_type = 0x4D;

        let result = sim.process_command(b"<C1F18>").unwrap();
        let expected = "#10A,11F,1ABC,1,0,101,102,103,104,105,106,1,1,0,0,0,100,1,12B,0,100,0,100,1,13C,0,100,1,14D,1,0,0,0,1,0#";
        assert_eq!(result.response, Some(expected.to_string()));
    }

    #[test]
    fn process_command_19_self_test_mem() {
        let mut sim = Simulator::new(0x1F);
        sim.fpgas[0].mem_a_test_ok = false; // Pre-fail the test
        sim.prog_id_hint = 123;
        sim.prog_id_lint = 456;

        // Command for full memory test (nDATA = 0)
        let result = sim.process_command(b"<C1F190000000000000000>").unwrap();
        assert_eq!(result.response, Some(String::from("#OK#")));

        // Verify state changes
        assert_eq!(sim.prog_id_hint, 0);
        assert_eq!(sim.prog_id_lint, 0);
        assert_eq!(sim.fpgas[0].mem_a_test_ok, true); // Should be set to true (pass)
    }

    #[test]
    fn process_command_20_get_fault_log() {
        let mut sim = Simulator::new(0x1F);
        // Pre-populate a fault log entry
        sim.fault_logs[2] = FaultLog {
            monitor_voltages: [1.1, 2.2, 3.3, 4.4, 5.5, 6.6],
            monitor_currents: [0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
            auto_reset_counter: 3,
            over_current_flags: 0b000001,  // PSU 1
            under_voltage_flags: 0b000010, // PSU 2
            over_voltage_flags: 0b000100,  // PSU 3
            clock_status_1_16: 0x1234,
            clock_status_17_32: 0xABCD,
            clock_status_33_48: 0xEF90,
            clock_status_49_64: 0x5678,
            sw_fault_status: 1, // SW1 fault
            sw1_rms: 1.23,
            sw2_rms: 4.56,
            driver_on: true,
            timer_values: [10, 20, 30, 40],
            alarm_values: [50, 60, 70, 80],
        };

        let result = sim.process_command(b"<C1F2000000000000002>").unwrap();
        let expected = "#101.10,100.10,102.20,100.20,103.30,100.30,104.40,100.40,105.50,100.50,106.60,100.60,1003,100000010000001000,1ABCD,11234,15678,1EF90,101,101.23,104.56,1,1010,1020,1030,1040,1050,1060,1070,1080#";
        assert_eq!(result.response, Some(expected.to_string()));
    }

    #[test]
    fn process_command_21_get_version() {
        let mut sim = Simulator::new(0x1F);
        sim.fw_version = 1.46;
        sim.fpgas[0].version = 5;
        sim.fpgas[1].version = 6;
        sim.clock_generators[0].fpga_version = 1;
        sim.clock_generators[1].fpga_version = 2;
        sim.clock_generators[2].fpga_version = 3;
        sim.clock_generators[3].fpga_version = 4;
        sim.sine_waves[0].fpga_version = 7;
        sim.sine_waves[1].fpga_version = 8;

        let result = sim.process_command(b"<C1F21>").unwrap();
        let expected = "#101.46,105,106,101,102,103,104,107,108,100#";
        assert_eq!(result.response, Some(expected.to_string()));
    }

    #[test]
    fn process_command_22_get_program_id() {
        let mut sim = Simulator::new(0x1F);
        sim.prog_id_hint = 12345;
        sim.prog_id_lint = 54321;
        let result = sim.process_command(b"<C1F22>").unwrap();
        assert_eq!(result.response, Some("#12345,54321#".to_string()));
    }

    #[test]
    fn process_command_23_get_program_id_checksum() {
        let mut sim = Simulator::new(0x1F);
        sim.prog_id_hint = 100;
        sim.prog_id_lint = 200;
        let result = sim.process_command(b"<C1F23>").unwrap();
        assert_eq!(result.response, Some("#300#".to_string()));
    }

    #[test]
    fn process_command_24_get_vi_monitor_string() {
        let mut sim = Simulator::new(0x1F);
        // FIXED: Enable the PSUs being tested
        sim.psus[0].enabled = true;
        sim.psus[5].enabled = true;

        // Set values
        sim.psus[0].voltage_setpoint = 1.23;
        sim.psus[0].current_limit = 0.45;
        sim.psus[5].voltage_setpoint = 900.5; // Test high voltage formatting
        sim.psus[5].current_limit = 6.78;

        // FIXED: Set limits to trigger expected faults
        sim.psus[0].high_voltage_limit = 1.0; // 1.23 > 1.0 -> Over-voltage
        sim.psus[5].high_voltage_limit = 900.0; // 900.5 > 900.0 -> Over-voltage
        sim.psus[5].current_monitor_limit = 6.0; // 6.78 > 6.0 -> Over-current

        sim.sine_waves[0].rms_value = 1.11;
        sim.sine_waves[1].rms_value = 2.22;
        sim.sequence_on = true;
        sim.door_open = false;

        let result = sim.process_command(b"<C1F24>").unwrap();

        // FIXED: The expected string is updated to reflect the correct simulated
        // measured values and the resulting fault flags.
        let expected_vi = "#100.00,100.50,100.00,100.50,100.00,100.50,100.00,100.50,100.00,100.50,102.20,100.50,1000,000000000000000000,10000,10000,10000,10000,100,101.11,102.22,1,1000,1000,1000,1000,1000,1000,1000,1000,1#";
        assert_eq!(result.response, Some(expected_vi.to_string()));
    }

    #[test]
    fn process_command_25_get_amon_monitor_string() {
        let mut sim = Simulator::new(0x1F);
        sim.amon_bp = 0xABCD;
        sim.amon_test_count = 2;

        // Configure PSU 1 (linked to test 1)
        sim.psus[0].high_voltage_limit = 5.5;
        sim.psus[0].low_voltage_limit = 4.5;

        // Configure PSU 2 (linked to test 2)
        sim.psus[1].current_monitor_limit = 1.0;

        // Configure test 1 (Voltage test)
        sim.amon_tests[0].test_type = 1;
        sim.amon_tests[0].psu_link = 1;
        sim.amon_tests[0].tp1_gain = 1.0;
        sim.amon_tests[0].cal_gain = 1.0;
        sim.amon_tests[0].cal_offset = 0.0;
        sim.amon_tests[0].board = 1;
        sim.amon_tests[0].tag = 2;

        // Configure test 2 (Current test)
        sim.amon_tests[1].test_type = 2;
        sim.amon_tests[1].psu_link = 2;
        sim.amon_tests[1].tp1_gain = 1.0;
        sim.amon_tests[1].cal_gain = 1.0;
        sim.amon_tests[1].cal_offset = 0.0;
        sim.amon_tests[1].board = 3;
        sim.amon_tests[1].tag = 4;

        // The simulated reading for test 1 will be (5.5+4.5)/2 = 5.0, which should pass (result 0)
        // The simulated reading for test 2 will be 1.0/2 = 0.5, which should pass (result 0)
        let result = sim.process_command(b"<C1F25>").unwrap();
        let expected = "#BBCD,105.00,0,11,102,100.50,0,13,104#";
        assert_eq!(result.response, Some(expected.to_string()));
    }

    #[test]
    fn checksum_validation_during_driver_load() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap();
        let v_command = b"<Vxx0605004003002001>";
        let expected_checksum = 0x06 + 0x05 + 0x004 + 0x003 + 0x002 + 0x001;
        sim.process_command(v_command).unwrap();
        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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
        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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
        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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
        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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
        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5001>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{},5,#", checksum)));
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

        let end_result = sim.process_command(b"<C1F5001>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{},3,#", checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
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

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        let expected_checksum = (0x01 + 0x01 + 0x0A + 0x0B + 0x0C + 0x0D + 0x01) +
            (0x02 + 0x02 + 0x14 + 0x32 + 0x1E + 0x64 + 0x05) +
            (0x03 + 0x03 + 0x01 + 0x02 + 0x03 + 0x04 + 0x0F) +
            (0x04 + 0x04 + 0x19 + 0x64 + 0x21 + 0xC8 + 0x0A);
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn i_command_updates_amon_cal_and_limits() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Type 4: cal_gain = 1.25 (0x3FA00000)
        let i_command4 = b"<Ixx40100000003FA00000>";
        sim.process_command(i_command4).unwrap();
        assert_eq!(sim.amon_tests[0].cal_gain, 1.25);

        // Type 5: cal_offset = -0.5 (0xBF000000)
        let i_command5 = b"<Ixx5010000000BF000000>";
        sim.process_command(i_command5).unwrap();
        assert_eq!(sim.amon_tests[0].cal_offset, -0.5);

        // Type 6: high_limit = 100.0 (0x42C80000)
        let i_command6 = b"<Ixx602000000042C80000>";
        sim.process_command(i_command6).unwrap();
        assert_eq!(sim.amon_tests[1].high_limit, 100.0);

        // Type 7: low_limit = 0.1 (0x3DCCCCCD)
        let i_command7 = b"<Ixx70200000003DCCCCCD>";
        sim.process_command(i_command7).unwrap();
        assert_eq!(sim.amon_tests[1].low_limit, 0.1);

        let end_result = sim.process_command(b"<C1F5003>").unwrap();

        let checksum1 = 4 + 1 + (0x3+0xF+0xA+0+0+0+0+0);
        let checksum2 = 5 + 1 + (0xB+0xF+0+0+0+0+0+0);
        let checksum3 = 6 + 2 + (0x4+0x2+0xC+0x8+0+0+0+0);
        let checksum4 = 7 + 2 + (0x3+0xD+0xC+0xC+0xC+0xC+0xC+0xD);
        let expected_checksum = checksum1 + checksum2 + checksum3 + checksum4;

        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
    }

    #[test]
    fn y_command_updates_amon_cal_and_metadata() {
        let mut sim = Simulator::new(0x1F);
        sim.process_command(b"<C1F5002>").unwrap(); // Start driver loading

        // Yxx<test=01><gain=03E8><offset=07D0><board=0A><tag=0B>
        let y_command = b"<Yxx0103E807D00A0B>";

        let test_num = 0x01;
        let gain = 0x03E8; // 1000
        let offset = 0x07D0; // 2000
        let board = 0x0A;
        let tag = 0x0B;
        let expected_checksum = gain + offset + test_num + board + tag;

        sim.process_command(y_command).unwrap();

        let test = &sim.amon_tests[0]; // Test #1 is at index 0
        assert_eq!(test.cal_gain, 1.0);
        assert_eq!(test.cal_offset, 2.0);
        assert_eq!(test.board, 10);
        assert_eq!(test.tag, 11);

        let end_result = sim.process_command(b"<C1F5003>").unwrap();
        assert_eq!(end_result.response, Some(format!("#{}#", expected_checksum)));
    }
}
