//! test helpers for generating minimal edf/bdf byte buffers

/// pad or truncate an ascii string to exactly `len` bytes, space-filled
fn ascii_field(s: &str, len: usize) -> Vec<u8> {
    let mut buf = vec![b' '; len];
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min(len);
    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
    buf
}

/// build a minimal valid EDF file with known signal data
///
/// returns the raw bytes of a complete EDF file with:
/// - 2 channels: "EEG1" (4 samples/record) and "EEG2" (2 samples/record)
/// - 3 data records, 1 second each
/// - known sample values for verification
pub fn minimal_edf() -> Vec<u8> {
    let num_signals = 2;
    let num_records = 3;
    let samples_per_record = [4_usize, 2];
    let header_bytes = 256 + num_signals * 256;

    let mut buf = Vec::with_capacity(4096);

    // -- fixed header (256 bytes) --
    buf.extend_from_slice(&ascii_field("0", 8)); // version
    buf.extend_from_slice(&ascii_field("test patient", 80)); // patient
    buf.extend_from_slice(&ascii_field("test recording", 80)); // recording
    buf.extend_from_slice(&ascii_field("01.02.24", 8)); // start date
    buf.extend_from_slice(&ascii_field("10.30.00", 8)); // start time
    buf.extend_from_slice(&ascii_field(&header_bytes.to_string(), 8)); // header bytes
    buf.extend_from_slice(&ascii_field("", 44)); // reserved
    buf.extend_from_slice(&ascii_field(&num_records.to_string(), 8)); // num data records
    buf.extend_from_slice(&ascii_field("1", 8)); // data record duration
    buf.extend_from_slice(&ascii_field(&num_signals.to_string(), 4)); // num signals

    // -- per-signal header fields (each field for all signals, then next field) --
    // labels
    buf.extend_from_slice(&ascii_field("EEG1", 16));
    buf.extend_from_slice(&ascii_field("EEG2", 16));
    // transducer type
    buf.extend_from_slice(&ascii_field("AgAgCl", 80));
    buf.extend_from_slice(&ascii_field("AgAgCl", 80));
    // physical dimension
    buf.extend_from_slice(&ascii_field("uV", 8));
    buf.extend_from_slice(&ascii_field("uV", 8));
    // physical min
    buf.extend_from_slice(&ascii_field("-500", 8));
    buf.extend_from_slice(&ascii_field("-500", 8));
    // physical max
    buf.extend_from_slice(&ascii_field("500", 8));
    buf.extend_from_slice(&ascii_field("500", 8));
    // digital min
    buf.extend_from_slice(&ascii_field("-32768", 8));
    buf.extend_from_slice(&ascii_field("-32768", 8));
    // digital max
    buf.extend_from_slice(&ascii_field("32767", 8));
    buf.extend_from_slice(&ascii_field("32767", 8));
    // prefiltering
    buf.extend_from_slice(&ascii_field("HP:0.1Hz", 80));
    buf.extend_from_slice(&ascii_field("HP:0.1Hz", 80));
    // num samples per record
    buf.extend_from_slice(&ascii_field(&samples_per_record[0].to_string(), 8));
    buf.extend_from_slice(&ascii_field(&samples_per_record[1].to_string(), 8));
    // reserved
    buf.extend_from_slice(&ascii_field("", 32));
    buf.extend_from_slice(&ascii_field("", 32));

    assert_eq!(buf.len(), header_bytes);

    // -- data records --
    // each record: samples_per_record[0] i16s for ch0, then samples_per_record[1] i16s for ch1
    // use simple ramp values so we can verify the reader
    for record in 0..num_records {
        // channel 0: 4 samples per record, values ramp up
        for s in 0..samples_per_record[0] {
            #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let val = (record * samples_per_record[0] + s) as i16 * 100;
            buf.extend_from_slice(&val.to_le_bytes());
        }
        // channel 1: 2 samples per record, negative ramp
        for s in 0..samples_per_record[1] {
            #[expect(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
            let val = -((record * samples_per_record[1] + s) as i16 * 200);
            buf.extend_from_slice(&val.to_le_bytes());
        }
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_edf_has_correct_size() {
        let data = minimal_edf();
        let header_size = 256 + 2 * 256;
        // 3 records × (4 + 2) samples × 2 bytes per sample
        let data_size = 3 * (4 + 2) * 2;
        assert_eq!(data.len(), header_size + data_size);
    }

    #[test]
    fn minimal_edf_starts_with_version() {
        let data = minimal_edf();
        assert_eq!(&data[..8], b"0       ");
    }
}
