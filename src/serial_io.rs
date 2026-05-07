use std::time::Duration;

use anyhow::{bail, Result};
use serialport::{DataBits, FlowControl, Parity, StopBits};

use crate::config::SerialConfig;

pub fn open_serial_reader(config: &SerialConfig) -> Result<Box<dyn serialport::SerialPort>> {
    open_port(config, &config.port)
}

pub fn open_serial_writer(
    config: &SerialConfig,
    port_override: &str,
) -> Result<Box<dyn serialport::SerialPort>> {
    open_port(config, port_override)
}

fn open_port(config: &SerialConfig, port: &str) -> Result<Box<dyn serialport::SerialPort>> {
    let builder = serialport::new(port, config.baud_rate)
        .timeout(Duration::from_millis(config.read_timeout_ms))
        .data_bits(to_data_bits(config.data_bits)?)
        .stop_bits(to_stop_bits(config.stop_bits)?)
        .parity(to_parity(&config.parity)?)
        .flow_control(to_flow_control(&config.flow_control)?);

    Ok(builder.open()?)
}

fn to_data_bits(value: u8) -> Result<DataBits> {
    match value {
        5 => Ok(DataBits::Five),
        6 => Ok(DataBits::Six),
        7 => Ok(DataBits::Seven),
        8 => Ok(DataBits::Eight),
        _ => bail!("unsupported data_bits: {value}"),
    }
}

fn to_stop_bits(value: u8) -> Result<StopBits> {
    match value {
        1 => Ok(StopBits::One),
        2 => Ok(StopBits::Two),
        _ => bail!("unsupported stop_bits: {value}"),
    }
}

fn to_parity(value: &str) -> Result<Parity> {
    match value {
        "none" => Ok(Parity::None),
        "odd" => Ok(Parity::Odd),
        "even" => Ok(Parity::Even),
        _ => bail!("unsupported parity: {value}"),
    }
}

fn to_flow_control(value: &str) -> Result<FlowControl> {
    match value {
        "none" => Ok(FlowControl::None),
        "software" => Ok(FlowControl::Software),
        "hardware" => Ok(FlowControl::Hardware),
        _ => bail!("unsupported flow_control: {value}"),
    }
}
