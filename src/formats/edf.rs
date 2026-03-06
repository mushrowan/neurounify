use crate::error::{Error, Result};
use crate::ir::Recording;
use std::path::Path;

use super::common;

const BYTES_PER_SAMPLE: usize = 2;

// -- reading --

pub fn read_bytes(data: &[u8]) -> Result<Recording> {
    let mut cursor = data;

    // version check: EDF starts with "0" padded to 8 bytes
    let version = common::read_ascii(&mut cursor, common::VERSION_LEN)?;
    if version != "0" {
        return Err(Error::InvalidHeader(format!(
            "expected EDF version \"0\", got {version:?}"
        )));
    }

    let parsed = common::parse_header(&mut cursor)?;
    let mut signals = parsed.signals;
    let data_section = cursor;

    let record_bytes: usize = parsed.samples_per_record.iter().sum::<usize>() * BYTES_PER_SAMPLE;
    let expected_len = record_bytes * parsed.header.num_data_records;
    if data_section.len() < expected_len {
        return Err(Error::InvalidData(format!(
            "expected {expected_len} bytes of sample data, got {}",
            data_section.len()
        )));
    }

    let mut pos = 0;
    for _ in 0..parsed.header.num_data_records {
        for (sig_idx, &n_samples) in parsed.samples_per_record.iter().enumerate() {
            for _ in 0..n_samples {
                let raw = i16::from_le_bytes([data_section[pos], data_section[pos + 1]]);
                let physical = signals[sig_idx].digital_to_physical(f64::from(raw));
                signals[sig_idx].samples.push(physical);
                pos += BYTES_PER_SAMPLE;
            }
        }
    }

    Ok(Recording {
        header: parsed.header,
        signals,
    })
}

pub fn read(path: &Path) -> Result<Recording> {
    let data = std::fs::read(path)?;
    read_bytes(&data)
}

// -- writing --

pub fn write_bytes(recording: &Recording) -> Result<Vec<u8>> {
    let samples_per_record: Vec<usize> = recording
        .signals
        .iter()
        .map(|s| s.samples_per_record(recording.header.data_record_duration))
        .collect();

    let mut buf = Vec::new();

    // EDF version
    common::write_ascii_field(&mut buf, "0", common::VERSION_LEN)?;
    common::write_header(&mut buf, recording, &samples_per_record)?;

    // data records: 16-bit little-endian samples
    for record in 0..recording.header.num_data_records {
        for (sig_idx, sig) in recording.signals.iter().enumerate() {
            let n = samples_per_record[sig_idx];
            let offset = record * n;
            for i in 0..n {
                let digital = sig.physical_to_digital(sig.samples[offset + i]);
                let clamped = digital.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX));
                #[expect(clippy::cast_possible_truncation)]
                let raw = clamped as i16;
                buf.extend_from_slice(&raw.to_le_bytes());
            }
        }
    }

    Ok(buf)
}

pub fn write(path: &Path, recording: &Recording) -> Result<()> {
    let data = write_bytes(recording)?;
    std::fs::write(path, data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::StartTime;
    use crate::testdata;

    #[test]
    fn read_minimal_edf_header() {
        let data = testdata::minimal_edf();
        let recording = read_bytes(&data).expect("should parse minimal edf");

        assert_eq!(recording.header.patient.as_deref(), Some("test patient"));
        assert_eq!(recording.header.recording.as_deref(), Some("test recording"));
        assert_eq!(recording.header.num_data_records, 3);
        assert!((recording.header.data_record_duration - 1.0).abs() < f64::EPSILON);
        assert_eq!(recording.num_channels(), 2);

        assert_eq!(
            recording.header.start_time,
            Some(StartTime {
                year: 2024,
                month: 2,
                day: 1,
                hour: 10,
                minute: 30,
                second: 0,
            })
        );
    }

    #[test]
    fn read_minimal_edf_signals() {
        let data = testdata::minimal_edf();
        let recording = read_bytes(&data).expect("should parse minimal edf");

        let sig0 = &recording.signals[0];
        assert_eq!(sig0.label, "EEG1");
        assert_eq!(sig0.physical_dimension, "uV");
        assert!((sig0.sample_rate - 4.0).abs() < f64::EPSILON);
        assert_eq!(sig0.samples.len(), 12);

        let sig1 = &recording.signals[1];
        assert_eq!(sig1.label, "EEG2");
        assert!((sig1.sample_rate - 2.0).abs() < f64::EPSILON);
        assert_eq!(sig1.samples.len(), 6);
    }

    #[test]
    fn read_minimal_edf_sample_values() {
        let data = testdata::minimal_edf();
        let recording = read_bytes(&data).expect("should parse minimal edf");

        let sig0 = &recording.signals[0];
        assert!(sig0.samples[0].abs() < 0.01, "sample 0: {}", sig0.samples[0]);
        assert!(sig0.samples[1] > 0.0);

        let sig1 = &recording.signals[1];
        assert!(sig1.samples[0].abs() < 0.01, "ch1 sample 0: {}", sig1.samples[0]);
        assert!(sig1.samples[1] < 0.0);
    }

    #[test]
    fn round_trip_minimal_edf() {
        let original_bytes = testdata::minimal_edf();
        let recording = read_bytes(&original_bytes).expect("read");
        let written_bytes = write_bytes(&recording).expect("write");
        let re_read = read_bytes(&written_bytes).expect("re-read");

        assert_eq!(recording.header.patient, re_read.header.patient);
        assert_eq!(recording.header.start_time, re_read.header.start_time);
        assert_eq!(recording.signals.len(), re_read.signals.len());
        for (orig, rt) in recording.signals.iter().zip(&re_read.signals) {
            assert_eq!(orig.label, rt.label);
            assert_eq!(orig.samples.len(), rt.samples.len());
            for (a, b) in orig.samples.iter().zip(&rt.samples) {
                assert!((a - b).abs() < 0.02, "sample mismatch: {a} vs {b}");
            }
        }
    }

    #[test]
    fn round_trip_bytes_identical() {
        let original = testdata::minimal_edf();
        let recording = read_bytes(&original).expect("read");
        let written = write_bytes(&recording).expect("write");
        assert_eq!(original.len(), written.len(), "file size mismatch");
        assert_eq!(original, written, "bytes differ after round-trip");
    }

    #[test]
    #[ignore] // run with: cargo test -- --ignored (needs testdata/fetch.sh)
    fn read_real_test_generator_edf() {
        let path = Path::new("testdata/test_generator.edf");
        if !path.exists() {
            eprintln!("skipping: run testdata/fetch.sh first");
            return;
        }
        let recording = read(path).expect("should parse test_generator.edf");
        assert_eq!(recording.num_channels(), 16);
        assert_eq!(recording.header.num_data_records, 900);
        assert!((recording.header.data_record_duration - 1.0).abs() < f64::EPSILON);
        assert!((recording.signals[0].sample_rate - 200.0).abs() < f64::EPSILON);
        assert_eq!(recording.signals[0].label, "F4");
    }
}
