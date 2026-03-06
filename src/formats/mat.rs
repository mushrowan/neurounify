use crate::error::{Error, Result};
use crate::ir::{Header, Recording, Signal};
use matrw::{matvar, load_matfile, save_matfile_v7, MatFile, MatVariable};
use std::path::Path;

// MAT layout:
//   data    - channels × samples double matrix (column-major)
//   labels  - cell array of channel label strings
//   srate   - scalar double (sample rate, from first channel)
//   patient - char array (optional)
//   recording - char array (optional)

// -- reading --

pub fn read(path: &Path) -> Result<Recording> {
    let path_str = path.to_str().ok_or_else(|| Error::Encoding("non-utf8 path".into()))?;
    let mat = load_matfile(path_str).map_err(|e| Error::InvalidData(format!("mat parse: {e}")))?;
    read_matfile(&mat)
}

fn read_matfile(mat: &MatFile) -> Result<Recording> {
    // data matrix: channels × samples (column-major)
    let data_var = &mat["data"];
    let dims = data_var.dim();
    if dims.len() < 2 {
        return Err(Error::InvalidData("data must be a 2D matrix".into()));
    }
    let n_channels = dims[0];
    let n_samples = dims[1];

    let flat = data_var
        .to_vec_f64()
        .ok_or_else(|| Error::InvalidData("data must be double".into()))?;

    // sample rate
    let srate = mat["srate"]
        .to_f64()
        .ok_or_else(|| Error::InvalidData("srate must be a scalar double".into()))?;

    // labels (optional)
    let has_labels = !matches!(mat["labels"], MatVariable::Null);
    let labels: Vec<String> = (0..n_channels)
        .map(|i| {
            if has_labels {
                let cell = &mat["labels"][i];
                if let Some(chars) = cell.to_vec_char() {
                    return chars.into_iter().collect();
                }
            }
            format!("ch{}", i + 1)
        })
        .collect();

    // optional metadata
    let patient = extract_string(&mat["patient"]);
    let recording = extract_string(&mat["recording"]);

    // unpack column-major data into per-channel sample vecs
    // column-major: flat[row + col * n_rows] = data[row][col]
    // so channel i, sample j = flat[i + j * n_channels]
    let signals: Vec<Signal> = (0..n_channels)
        .map(|ch| {
            let samples: Vec<f64> = (0..n_samples)
                .map(|s| flat[ch + s * n_channels])
                .collect();
            Signal {
                label: labels[ch].clone(),
                transducer: String::new(),
                physical_dimension: String::new(),
                physical_min: samples.iter().copied().fold(f64::INFINITY, f64::min),
                physical_max: samples.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                digital_min: -32768,
                digital_max: 32767,
                prefiltering: String::new(),
                sample_rate: srate,
                samples,
            }
        })
        .collect();

    let num_data_records = if srate > 0.0 {
        #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
        let n = (n_samples as f64 / srate).ceil() as usize;
        n
    } else {
        1
    };

    Ok(Recording {
        header: Header {
            patient,
            recording,
            start_time: None,
            data_record_duration: 1.0,
            num_data_records,
        },
        signals,
    })
}

fn extract_string(var: &MatVariable) -> Option<String> {
    var.to_vec_char()
        .map(|chars| chars.into_iter().collect::<String>())
        .filter(|s| !s.is_empty())
}

// -- writing --

pub fn write(path: &Path, recording: &Recording) -> Result<()> {
    let path_str = path.to_str().ok_or_else(|| Error::Encoding("non-utf8 path".into()))?;
    let mat = build_matfile(recording)?;
    save_matfile_v7(path_str, mat, true)
        .map_err(|e| Error::InvalidData(format!("mat write: {e}")))?;
    Ok(())
}

fn build_matfile(recording: &Recording) -> Result<MatFile> {
    let n_channels = recording.signals.len();
    if n_channels == 0 {
        return Err(Error::InvalidData("no signals to write".into()));
    }
    let n_samples = recording.signals[0].samples.len();

    // build column-major flat vec: data[ch + sample * n_channels]
    let mut flat = vec![0.0_f64; n_channels * n_samples];
    for (ch, sig) in recording.signals.iter().enumerate() {
        for (s, &val) in sig.samples.iter().enumerate() {
            flat[ch + s * n_channels] = val;
        }
    }

    // build the data matrix
    let numeric = matrw::NumericArray::new(
        vec![n_channels, n_samples],
        matrw::MatlabType::F64(flat),
        None,
    ).map_err(|e| Error::InvalidData(format!("numeric array: {e}")))?;
    let data_var = MatVariable::NumericArray(numeric);

    // labels as cell array of strings
    let label_cells: Vec<MatVariable> = recording
        .signals
        .iter()
        .map(|sig| matvar!(sig.label.as_str()))
        .collect();
    let cell = matrw::CellArray::new(vec![1, n_channels], label_cells)
        .map_err(|e| Error::InvalidData(format!("cell array: {e}")))?;
    let labels_var = MatVariable::CellArray(cell);

    // sample rate (from first signal)
    let srate = recording.signals[0].sample_rate;

    let mut mat = MatFile::new();
    mat.insert("data", data_var);
    mat.insert("labels", labels_var);
    mat.insert("srate", matvar!(srate));

    if let Some(patient) = &recording.header.patient {
        mat.insert("patient", matvar!(patient.as_str()));
    }
    if let Some(rec) = &recording.header.recording {
        mat.insert("recording", matvar!(rec.as_str()));
    }

    Ok(mat)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata;

    fn recording_from_edf() -> Recording {
        let data = testdata::minimal_edf();
        crate::formats::edf::read_bytes(&data).expect("read edf")
    }

    #[test]
    fn edf_to_mat_round_trip() {
        let original = recording_from_edf();

        let tmp = tempfile::NamedTempFile::with_suffix(".mat").expect("tempfile");
        write(tmp.path(), &original).expect("write mat");

        let re_read = read(tmp.path()).expect("read mat");

        assert_eq!(re_read.num_channels(), original.num_channels());
        assert_eq!(re_read.signals[0].label, "EEG1");
        assert_eq!(re_read.signals[1].label, "EEG2");

        assert_eq!(
            re_read.signals[0].samples.len(),
            original.signals[0].samples.len()
        );

        for (orig, rt) in original.signals.iter().zip(&re_read.signals) {
            for (a, b) in orig.samples.iter().zip(&rt.samples) {
                assert!(
                    (a - b).abs() < 0.1,
                    "sample mismatch: {a} vs {b}"
                );
            }
        }
    }

    #[test]
    fn mat_preserves_metadata() {
        let original = recording_from_edf();

        let tmp = tempfile::NamedTempFile::with_suffix(".mat").expect("tempfile");
        write(tmp.path(), &original).expect("write mat");
        let re_read = read(tmp.path()).expect("read mat");

        assert_eq!(re_read.header.patient, original.header.patient);
        assert_eq!(re_read.header.recording, original.header.recording);
    }
}
