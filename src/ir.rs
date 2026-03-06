//! intermediate representation for eeg recordings
//!
//! all sample data is stored as physical (real-world) values in f64,
//! regardless of the source format's native encoding

/// a complete eeg recording
#[derive(Debug, Clone)]
pub struct Recording {
    pub header: Header,
    pub signals: Vec<Signal>,
}

/// recording-level metadata
#[derive(Debug, Clone, Default)]
pub struct Header {
    pub patient: Option<String>,
    pub recording: Option<String>,
    pub start_time: Option<StartTime>,
    /// duration of one data record in seconds (edf/bdf concept,
    /// preserved for round-trip fidelity)
    pub data_record_duration: f64,
    /// number of data records (derived from signal lengths on write)
    pub num_data_records: usize,
}

/// start date and time of the recording
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

/// a single signal (channel) in the recording
#[derive(Debug, Clone)]
pub struct Signal {
    pub label: String,
    pub transducer: String,
    pub physical_dimension: String,
    pub physical_min: f64,
    pub physical_max: f64,
    pub digital_min: i32,
    pub digital_max: i32,
    pub prefiltering: String,
    /// samples per second
    pub sample_rate: f64,
    /// sample values in physical units
    pub samples: Vec<f64>,
}

impl Signal {
    /// number of samples per data record
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn samples_per_record(&self, record_duration: f64) -> usize {
        (self.sample_rate * record_duration) as usize
    }

    /// convert a physical value to a digital (integer) value
    pub fn physical_to_digital(&self, physical: f64) -> f64 {
        let phys_range = self.physical_max - self.physical_min;
        let dig_range = f64::from(self.digital_max - self.digital_min);
        if phys_range == 0.0 {
            return f64::from(self.digital_min);
        }
        f64::from(self.digital_min) + (physical - self.physical_min) * dig_range / phys_range
    }

    /// convert a digital (integer) value to a physical value
    pub fn digital_to_physical(&self, digital: f64) -> f64 {
        let phys_range = self.physical_max - self.physical_min;
        let dig_range = f64::from(self.digital_max - self.digital_min);
        if dig_range == 0.0 {
            return self.physical_min;
        }
        self.physical_min + (digital - f64::from(self.digital_min)) * phys_range / dig_range
    }
}

impl Recording {
    /// total duration of the recording in seconds
    #[expect(clippy::cast_precision_loss)]
    pub fn duration(&self) -> f64 {
        self.header.data_record_duration * self.header.num_data_records as f64
    }

    /// number of channels
    pub fn num_channels(&self) -> usize {
        self.signals.len()
    }
}
