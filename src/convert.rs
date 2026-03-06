use crate::error::{Error, Result};
use crate::formats::{bdf, edf, mat};
use crate::ir::Recording;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Edf,
    Bdf,
    Mat,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edf => write!(f, "EDF"),
            Self::Bdf => write!(f, "BDF"),
            Self::Mat => write!(f, "MAT"),
        }
    }
}

impl Format {
    /// detect format from file extension
    pub fn from_extension(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_lowercase);
        match ext.as_deref() {
            Some("edf") => Some(Self::Edf),
            Some("bdf") => Some(Self::Bdf),
            Some("mat") => Some(Self::Mat),
            _ => None,
        }
    }

    /// detect format from the first few bytes of file content
    pub fn from_magic(data: &[u8]) -> Option<Self> {
        if data.len() >= 8 {
            // BDF: 0xFF followed by "BIOSEMI"
            if data[0] == 0xFF && &data[1..8] == b"BIOSEMI" {
                return Some(Self::Bdf);
            }
            // EDF: starts with "0" (0x30)
            if data[0] == b'0' {
                return Some(Self::Edf);
            }
        }
        // MAT v5: starts with "MATLAB" in the header text
        if data.len() >= 6 && &data[..6] == b"MATLAB" {
            return Some(Self::Mat);
        }
        None
    }

    /// detect format: try extension first, fall back to magic bytes
    pub fn detect(path: &Path) -> Result<Self> {
        if let Some(fmt) = Self::from_extension(path) {
            return Ok(fmt);
        }
        // read first 8 bytes for magic detection
        let data = std::fs::read(path).map_err(|_| Error::UnsupportedFormat(path.to_path_buf()))?;
        Self::from_magic(&data).ok_or_else(|| Error::UnsupportedFormat(path.to_path_buf()))
    }
}

pub fn read(path: &Path) -> Result<Recording> {
    let format = Format::detect(path)?;
    match format {
        Format::Edf => edf::read(path),
        Format::Bdf => bdf::read(path),
        Format::Mat => mat::read(path),
    }
}

pub fn write(path: &Path, recording: &Recording) -> Result<()> {
    let format = Format::from_extension(path)
        .ok_or_else(|| Error::UnsupportedFormat(path.to_path_buf()))?;
    match format {
        Format::Edf => edf::write(path, recording),
        Format::Bdf => bdf::write(path, recording),
        Format::Mat => mat::write(path, recording),
    }
}

pub fn convert(input: &Path, output: &Path) -> Result<()> {
    let recording = read(input)?;
    write(output, &recording)
}

/// parse and validate a file, returning the recording and detected format
pub fn check(path: &Path) -> Result<(Format, Recording)> {
    let format = Format::detect(path)?;
    let recording = match format {
        Format::Edf => edf::read(path)?,
        Format::Bdf => bdf::read(path)?,
        Format::Mat => mat::read(path)?,
    };
    Ok((format, recording))
}

/// print a human-readable summary of a recording
pub fn print_info(path: &Path, format: Format, recording: &Recording) {
    println!("file:       {}", path.display());
    println!("format:     {format}");
    if let Some(p) = &recording.header.patient {
        println!("patient:    {p}");
    }
    if let Some(r) = &recording.header.recording {
        println!("recording:  {r}");
    }
    if let Some(st) = &recording.header.start_time {
        println!(
            "start:      {:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            st.year, st.month, st.day, st.hour, st.minute, st.second
        );
    }
    println!("channels:   {}", recording.num_channels());
    println!("records:    {}", recording.header.num_data_records);
    println!(
        "duration:   {:.1}s ({:.1}s per record)",
        recording.duration(),
        recording.header.data_record_duration
    );
    println!();
    println!("  {:<16} {:>8} {:>10} {:>10}  unit", "label", "rate", "min", "max");
    println!("  {}", "-".repeat(60));
    for sig in &recording.signals {
        let n = sig.samples.len();
        let (lo, hi) = if n > 0 {
            let lo = sig.samples.iter().copied().fold(f64::INFINITY, f64::min);
            let hi = sig.samples.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            (lo, hi)
        } else {
            (0.0, 0.0)
        };
        println!(
            "  {:<16} {:>7.0}Hz {:>10.2} {:>10.2}  {}",
            sig.label, sig.sample_rate, lo, hi, sig.physical_dimension
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_edf_by_magic() {
        let mut data = vec![0u8; 256];
        data[0] = b'0';
        assert_eq!(Format::from_magic(&data), Some(Format::Edf));
    }

    #[test]
    fn detect_bdf_by_magic() {
        let mut data = vec![0u8; 256];
        data[0] = 0xFF;
        data[1..8].copy_from_slice(b"BIOSEMI");
        assert_eq!(Format::from_magic(&data), Some(Format::Bdf));
    }

    #[test]
    fn detect_mat_by_magic() {
        let mut data = vec![0u8; 128];
        data[..20].copy_from_slice(b"MATLAB 5.0 MAT-file ");
        assert_eq!(Format::from_magic(&data), Some(Format::Mat));
    }

    #[test]
    fn detect_format_from_extension() {
        assert_eq!(
            Format::from_extension(Path::new("foo.edf")),
            Some(Format::Edf)
        );
        assert_eq!(
            Format::from_extension(Path::new("foo.BDF")),
            Some(Format::Bdf)
        );
        assert_eq!(
            Format::from_extension(Path::new("foo.mat")),
            Some(Format::Mat)
        );
        assert_eq!(Format::from_extension(Path::new("foo.xyz")), None);
    }
}
