//! shared header parsing and writing for EDF/BDF formats
//!
//! both formats have identical header structure, differing only in the
//! version field and bytes per sample in data records

use crate::error::{Error, Result};
use crate::ir::{Header, Recording, Signal, StartTime};
use std::io::{self, Write};

// header field sizes (same for EDF and BDF)
pub const VERSION_LEN: usize = 8;
pub const PATIENT_LEN: usize = 80;
pub const RECORDING_LEN: usize = 80;
pub const DATE_LEN: usize = 8;
pub const TIME_LEN: usize = 8;
pub const HEADER_BYTES_LEN: usize = 8;
pub const RESERVED_LEN: usize = 44;
pub const NUM_RECORDS_LEN: usize = 8;
pub const DURATION_LEN: usize = 8;
pub const NUM_SIGNALS_LEN: usize = 4;
pub const FIXED_HEADER_LEN: usize = 256;

// per-signal field sizes
pub const LABEL_LEN: usize = 16;
pub const TRANSDUCER_LEN: usize = 80;
pub const PHYS_DIM_LEN: usize = 8;
pub const PHYS_MIN_LEN: usize = 8;
pub const PHYS_MAX_LEN: usize = 8;
pub const DIG_MIN_LEN: usize = 8;
pub const DIG_MAX_LEN: usize = 8;
pub const PREFILTER_LEN: usize = 80;
pub const SAMPLES_LEN: usize = 8;
pub const SIGNAL_RESERVED_LEN: usize = 32;

// -- reading helpers --

/// read and trim an ascii field from a byte cursor
pub fn read_ascii(cursor: &mut &[u8], len: usize) -> Result<String> {
    if cursor.len() < len {
        return Err(Error::InvalidHeader("unexpected end of header".into()));
    }
    let (field, rest) = cursor.split_at(len);
    *cursor = rest;
    String::from_utf8(field.to_vec())
        .map(|s| s.trim_end().to_owned())
        .map_err(|e| Error::Encoding(e.to_string()))
}

/// read raw bytes from cursor (for version field which may contain non-ascii)
pub fn read_raw(cursor: &mut &[u8], len: usize) -> Result<Vec<u8>> {
    if cursor.len() < len {
        return Err(Error::InvalidHeader("unexpected end of header".into()));
    }
    let (field, rest) = cursor.split_at(len);
    *cursor = rest;
    Ok(field.to_vec())
}

pub fn parse_f64(s: &str, field_name: &str) -> Result<f64> {
    s.trim()
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad {field_name}: {s:?}")))
}

pub fn parse_i32(s: &str, field_name: &str) -> Result<i32> {
    s.trim()
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad {field_name}: {s:?}")))
}

pub fn parse_usize(s: &str, field_name: &str) -> Result<usize> {
    s.trim()
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad {field_name}: {s:?}")))
}

pub fn parse_start_time(date_str: &str, time_str: &str) -> Result<StartTime> {
    let date_parts: Vec<&str> = date_str.split('.').collect();
    let time_parts: Vec<&str> = time_str.split('.').collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return Err(Error::InvalidHeader(format!(
            "bad date/time: {date_str:?} {time_str:?}"
        )));
    }

    let day: u8 = date_parts[0]
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad day: {:?}", date_parts[0])))?;
    let month: u8 = date_parts[1]
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad month: {:?}", date_parts[1])))?;
    let yy: u16 = date_parts[2]
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad year: {:?}", date_parts[2])))?;
    // 2-digit years: 85+ means 1985, otherwise 2000+
    let year = if yy >= 85 { 1900 + yy } else { 2000 + yy };

    let hour: u8 = time_parts[0]
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad hour: {:?}", time_parts[0])))?;
    let minute: u8 = time_parts[1]
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad minute: {:?}", time_parts[1])))?;
    let second: u8 = time_parts[2]
        .parse()
        .map_err(|_| Error::InvalidHeader(format!("bad second: {:?}", time_parts[2])))?;

    Ok(StartTime {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

fn read_signal_strings(cursor: &mut &[u8], ns: usize, field_len: usize) -> Result<Vec<String>> {
    (0..ns).map(|_| read_ascii(cursor, field_len)).collect()
}

/// parsed header fields needed to build a Recording (before data decoding)
pub struct ParsedHeader {
    pub header: Header,
    pub signals: Vec<Signal>,
    pub samples_per_record: Vec<usize>,
}

/// parse signal header fields after the version byte has been consumed
///
/// `cursor` should be positioned right after the 8-byte version field
pub fn parse_header(cursor: &mut &[u8]) -> Result<ParsedHeader> {
    let patient = read_ascii(cursor, PATIENT_LEN)?;
    let recording = read_ascii(cursor, RECORDING_LEN)?;
    let date_str = read_ascii(cursor, DATE_LEN)?;
    let time_str = read_ascii(cursor, TIME_LEN)?;
    let _header_bytes = read_ascii(cursor, HEADER_BYTES_LEN)?;
    let _reserved = read_ascii(cursor, RESERVED_LEN)?;
    let num_data_records = parse_usize(&read_ascii(cursor, NUM_RECORDS_LEN)?, "num_data_records")?;
    let data_record_duration = parse_f64(&read_ascii(cursor, DURATION_LEN)?, "duration")?;
    let ns = parse_usize(&read_ascii(cursor, NUM_SIGNALS_LEN)?, "num_signals")?;

    let start_time = parse_start_time(&date_str, &time_str).ok();

    // per-signal fields
    let labels = read_signal_strings(cursor, ns, LABEL_LEN)?;
    let transducers = read_signal_strings(cursor, ns, TRANSDUCER_LEN)?;
    let phys_dims = read_signal_strings(cursor, ns, PHYS_DIM_LEN)?;
    let phys_mins: Vec<f64> = read_signal_strings(cursor, ns, PHYS_MIN_LEN)?
        .iter()
        .enumerate()
        .map(|(i, s)| parse_f64(s, &format!("physical_min[{i}]")))
        .collect::<Result<_>>()?;
    let phys_maxs: Vec<f64> = read_signal_strings(cursor, ns, PHYS_MAX_LEN)?
        .iter()
        .enumerate()
        .map(|(i, s)| parse_f64(s, &format!("physical_max[{i}]")))
        .collect::<Result<_>>()?;
    let dig_mins: Vec<i32> = read_signal_strings(cursor, ns, DIG_MIN_LEN)?
        .iter()
        .enumerate()
        .map(|(i, s)| parse_i32(s, &format!("digital_min[{i}]")))
        .collect::<Result<_>>()?;
    let dig_maxs: Vec<i32> = read_signal_strings(cursor, ns, DIG_MAX_LEN)?
        .iter()
        .enumerate()
        .map(|(i, s)| parse_i32(s, &format!("digital_max[{i}]")))
        .collect::<Result<_>>()?;
    let prefilters = read_signal_strings(cursor, ns, PREFILTER_LEN)?;
    let samples_per_record: Vec<usize> = read_signal_strings(cursor, ns, SAMPLES_LEN)?
        .iter()
        .enumerate()
        .map(|(i, s)| parse_usize(s, &format!("samples_per_record[{i}]")))
        .collect::<Result<_>>()?;
    let _signal_reserved = read_signal_strings(cursor, ns, SIGNAL_RESERVED_LEN)?;

    let signals: Vec<Signal> = (0..ns)
        .map(|i| {
            let sample_rate = if data_record_duration > 0.0 {
                #[expect(clippy::cast_precision_loss)]
                let rate = samples_per_record[i] as f64 / data_record_duration;
                rate
            } else {
                0.0
            };
            Signal {
                label: labels[i].clone(),
                transducer: transducers[i].clone(),
                physical_dimension: phys_dims[i].clone(),
                physical_min: phys_mins[i],
                physical_max: phys_maxs[i],
                digital_min: dig_mins[i],
                digital_max: dig_maxs[i],
                prefiltering: prefilters[i].clone(),
                sample_rate,
                samples: Vec::with_capacity(samples_per_record[i] * num_data_records),
            }
        })
        .collect();

    Ok(ParsedHeader {
        header: Header {
            patient: Some(patient).filter(|s| !s.is_empty()),
            recording: Some(recording).filter(|s| !s.is_empty()),
            start_time,
            data_record_duration,
            num_data_records,
        },
        signals,
        samples_per_record,
    })
}

// -- writing helpers --

pub fn write_ascii_field(w: &mut impl Write, s: &str, len: usize) -> io::Result<()> {
    let bytes = s.as_bytes();
    let write_len = bytes.len().min(len);
    w.write_all(&bytes[..write_len])?;
    for _ in write_len..len {
        w.write_all(b" ")?;
    }
    Ok(())
}

/// write the common header fields (everything after version, which the caller writes)
pub fn write_header(buf: &mut Vec<u8>, recording: &Recording, samples_per_record: &[usize]) -> Result<()> {
    let ns = recording.signals.len();

    write_ascii_field(buf, recording.header.patient.as_deref().unwrap_or(""), PATIENT_LEN)?;
    write_ascii_field(buf, recording.header.recording.as_deref().unwrap_or(""), RECORDING_LEN)?;

    if let Some(st) = &recording.header.start_time {
        write_ascii_field(buf, &format!("{:02}.{:02}.{:02}", st.day, st.month, st.year % 100), DATE_LEN)?;
        write_ascii_field(buf, &format!("{:02}.{:02}.{:02}", st.hour, st.minute, st.second), TIME_LEN)?;
    } else {
        write_ascii_field(buf, "", DATE_LEN)?;
        write_ascii_field(buf, "", TIME_LEN)?;
    }

    let header_bytes = FIXED_HEADER_LEN + ns * 256;
    write_ascii_field(buf, &header_bytes.to_string(), HEADER_BYTES_LEN)?;
    write_ascii_field(buf, "", RESERVED_LEN)?;
    write_ascii_field(buf, &recording.header.num_data_records.to_string(), NUM_RECORDS_LEN)?;
    write_ascii_field(buf, &format!("{}", recording.header.data_record_duration), DURATION_LEN)?;
    write_ascii_field(buf, &ns.to_string(), NUM_SIGNALS_LEN)?;

    // per-signal fields
    for sig in &recording.signals {
        write_ascii_field(buf, &sig.label, LABEL_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &sig.transducer, TRANSDUCER_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &sig.physical_dimension, PHYS_DIM_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &format!("{}", sig.physical_min), PHYS_MIN_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &format!("{}", sig.physical_max), PHYS_MAX_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &format!("{}", sig.digital_min), DIG_MIN_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &format!("{}", sig.digital_max), DIG_MAX_LEN)?;
    }
    for sig in &recording.signals {
        write_ascii_field(buf, &sig.prefiltering, PREFILTER_LEN)?;
    }
    for &spr in samples_per_record {
        write_ascii_field(buf, &spr.to_string(), SAMPLES_LEN)?;
    }
    for _ in &recording.signals {
        write_ascii_field(buf, "", SIGNAL_RESERVED_LEN)?;
    }

    Ok(())
}
