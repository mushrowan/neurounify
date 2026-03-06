use crate::error::{Error, Result};
use crate::ir::Recording;
use std::path::Path;

use super::common;

const BYTES_PER_SAMPLE: usize = 3;

/// BDF version: first byte 0xFF, then "BIOSEMI"
const BDF_VERSION: &[u8] = b"\xffBIOSEMI";

/// decode a 24-bit little-endian signed integer from 3 bytes
fn decode_i24(b: [u8; 3]) -> i32 {
    let unsigned = i32::from(b[0]) | (i32::from(b[1]) << 8) | (i32::from(b[2]) << 16);
    // sign-extend from 24 bits
    if unsigned & 0x80_0000 != 0 {
        unsigned | !0xFF_FFFF
    } else {
        unsigned
    }
}

/// encode an i32 as 24-bit little-endian (3 bytes), clamping to 24-bit range
fn encode_i24(val: i32) -> [u8; 3] {
    let clamped = val.clamp(-8_388_608, 8_388_607);
    #[expect(clippy::cast_sign_loss)]
    let unsigned = clamped as u32;
    #[expect(clippy::cast_possible_truncation)]
    [unsigned as u8, (unsigned >> 8) as u8, (unsigned >> 16) as u8]
}

// -- reading --

pub fn read_bytes(data: &[u8]) -> Result<Recording> {
    let mut cursor = data;

    // version check: BDF starts with 0xFF "BIOSEMI"
    let version_raw = common::read_raw(&mut cursor, common::VERSION_LEN)?;
    if version_raw != BDF_VERSION {
        return Err(Error::InvalidHeader(format!(
            "expected BDF version (0xFF BIOSEMI), got {version_raw:?}"
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
                let raw = decode_i24([
                    data_section[pos],
                    data_section[pos + 1],
                    data_section[pos + 2],
                ]);
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

    // BDF version: 0xFF + "BIOSEMI"
    buf.extend_from_slice(BDF_VERSION);
    common::write_header(&mut buf, recording, &samples_per_record)?;

    // data records: 24-bit little-endian samples
    for record in 0..recording.header.num_data_records {
        for (sig_idx, sig) in recording.signals.iter().enumerate() {
            let n = samples_per_record[sig_idx];
            let offset = record * n;
            for i in 0..n {
                let digital = sig.physical_to_digital(sig.samples[offset + i]);
                #[expect(clippy::cast_possible_truncation)]
                let raw = digital.round() as i32;
                buf.extend_from_slice(&encode_i24(raw));
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
    use crate::testdata;

    #[test]
    fn i24_round_trip() {
        for val in [-8_388_608, -1, 0, 1, 8_388_607, 1000, -1000] {
            assert_eq!(decode_i24(encode_i24(val)), val, "failed for {val}");
        }
    }

    #[test]
    fn i24_sign_extension() {
        // -1 in 24-bit is 0xFF_FFFF
        assert_eq!(decode_i24([0xFF, 0xFF, 0xFF]), -1);
        // -128 in 24-bit is 0xFF_FF80
        assert_eq!(decode_i24([0x80, 0xFF, 0xFF]), -128);
        // max positive: 0x7F_FFFF = 8388607
        assert_eq!(decode_i24([0xFF, 0xFF, 0x7F]), 8_388_607);
    }

    #[test]
    fn edf_to_bdf_round_trip() {
        // read as EDF, write as BDF, read back as BDF
        let edf_data = testdata::minimal_edf();
        let recording = crate::formats::edf::read_bytes(&edf_data).expect("read edf");

        // adjust digital range to 24-bit for BDF output
        let mut bdf_recording = recording.clone();
        for sig in &mut bdf_recording.signals {
            sig.digital_min = -8_388_608;
            sig.digital_max = 8_388_607;
        }

        let bdf_bytes = write_bytes(&bdf_recording).expect("write bdf");
        let re_read = read_bytes(&bdf_bytes).expect("read bdf");

        assert_eq!(re_read.num_channels(), 2);
        assert_eq!(re_read.signals[0].label, "EEG1");
        assert_eq!(re_read.signals[0].samples.len(), 12);

        // sample values should be close (within quantisation error)
        for (orig, rt) in bdf_recording.signals.iter().zip(&re_read.signals) {
            for (a, b) in orig.samples.iter().zip(&rt.samples) {
                assert!((a - b).abs() < 0.01, "sample mismatch: {a} vs {b}");
            }
        }
    }

    #[test]
    #[ignore] // needs testdata/fetch.sh
    fn read_real_test_generator_bdf() {
        let path = Path::new("testdata/test_generator_2.bdf");
        if !path.exists() {
            eprintln!("skipping: run testdata/fetch.sh first");
            return;
        }
        let recording = read(path).expect("should parse test_generator_2.bdf");
        // this is a BDF+ file with 11 signal channels + 1 annotations channel
        assert_eq!(recording.num_channels(), 12);
        assert_eq!(recording.header.num_data_records, 600);
        assert!((recording.header.data_record_duration - 1.0).abs() < f64::EPSILON);
    }
}
