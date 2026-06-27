// SPDX-License-Identifier: AGPL-3.0-only

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::config::Config;

/// Stream `reader` to `writer` in bounded chunks, honouring optional bandwidth
/// limits and cooperative cancellation. Updates `progress` when provided.
pub fn copy_with_limits(
    reader: &mut impl Read,
    writer: &mut impl Write,
    cancel: &AtomicBool,
    chunk_bytes: usize,
    bandwidth_bps: Option<u64>,
    progress: Option<&AtomicU64>,
) -> io::Result<u64> {
    let chunk_bytes = chunk_bytes.clamp(16 * 1024, 4 * 1024 * 1024);
    let mut buf = vec![0u8; chunk_bytes];
    let mut total = 0u64;
    let mut window_start = Instant::now();
    let mut window_bytes = 0u64;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Ok(total);
        }

        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }

        writer.write_all(&buf[..n])?;
        total += n as u64;
        if let Some(counter) = progress {
            counter.fetch_add(n as u64, Ordering::Relaxed);
        }

        if let Some(bps) = bandwidth_bps.filter(|bps| *bps > 0) {
            window_bytes += n as u64;
            let elapsed = window_start.elapsed();
            let expected = Duration::from_secs_f64(window_bytes as f64 / bps as f64);
            if expected > elapsed {
                std::thread::sleep(expected - elapsed);
            }
            if elapsed >= Duration::from_secs(1) {
                window_start = Instant::now();
                window_bytes = 0;
            }
        }
    }

    Ok(total)
}

pub fn copy_file_with_limits(
    config: &Config,
    src: &Path,
    dst: &Path,
    cancel: &AtomicBool,
    progress: Option<&AtomicU64>,
) -> Result<()> {
    let mut src_file = File::open(src).with_context(|| format!("open {}", src.display()))?;
    let mut dst_file = File::create(dst).with_context(|| format!("create {}", dst.display()))?;
    copy_with_limits(
        &mut src_file,
        &mut dst_file,
        cancel,
        config.model_download_write_chunk_bytes,
        config.model_download_bandwidth_bps,
        progress,
    )
    .with_context(|| format!("copy {} to {}", src.display(), dst.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn copy_with_limits_respects_cancel() {
        let data = vec![0u8; 1024];
        let mut reader = Cursor::new(data);
        let mut writer = Vec::new();
        let cancel = AtomicBool::new(true);
        let copied = copy_with_limits(&mut reader, &mut writer, &cancel, 256, None, None).unwrap();
        assert_eq!(copied, 0);
        assert!(writer.is_empty());
    }

    #[test]
    fn copy_with_limits_copies_all_bytes() {
        let data = vec![7u8; 1024];
        let mut reader = Cursor::new(data.clone());
        let mut writer = Vec::new();
        let cancel = AtomicBool::new(false);
        let progress = AtomicU64::new(0);
        let copied = copy_with_limits(
            &mut reader,
            &mut writer,
            &cancel,
            128,
            None,
            Some(&progress),
        )
        .unwrap();
        assert_eq!(copied, 1024);
        assert_eq!(writer, data);
        assert_eq!(progress.load(Ordering::Relaxed), 1024);
    }
}
