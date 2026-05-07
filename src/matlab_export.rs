use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use anyhow::Result;
use byteorder::{LittleEndian, WriteBytesExt};

use crate::log_format::CaptureLog;

pub fn export_matlab(log: &CaptureLog, out_dir: impl AsRef<Path>) -> Result<()> {
    let out_dir = out_dir.as_ref();
    fs::create_dir_all(out_dir)?;

    for channel in &log.channels {
        let channel_dir = out_dir.join(&channel.name);
        fs::create_dir_all(&channel_dir)?;

        let mut data = BufWriter::new(File::create(channel_dir.join("data.bin"))?);
        let mut timestamps = BufWriter::new(File::create(channel_dir.join("timestamps_ns.bin"))?);

        for record in log
            .records
            .iter()
            .filter(|record| record.channel_id == channel.id)
        {
            data.write_all(&record.packet)?;
            timestamps.write_u64::<LittleEndian>(record.timestamp_unix_ns)?;
        }
    }

    Ok(())
}
