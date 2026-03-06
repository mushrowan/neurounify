# neurounify

universal eeg format converter in rust

reads and writes EDF, BDF, and MAT files

## install

### nix

```bash
nix run github:mushrowan/neurounify -- --help
```

### cargo

```bash
cargo install --git https://github.com/mushrowan/neurounify
```

## usage

```bash
# convert between formats (auto-detected from extensions)
neurounify recording.edf recording.bdf
neurounify recording.bdf recording.mat
neurounify recording.mat output.edf

# inspect a file without converting
neurounify recording.edf
neurounify --check recording.bdf
```

example output:

```
file:       recording.edf
format:     EDF
patient:    test file
recording:  EDF generator
start:      2008-10-02 14:27:00
channels:   16
records:    900
duration:   900.0s (1.0s per record)

  label                rate        min        max  unit
  ------------------------------------------------------------
  F4                   200Hz    -799.96     800.06  uV
  F3                   100Hz    -800.74     800.84  uV
  X10                  200Hz      -0.80       0.80  mV
  FP2                  200Hz   -3200.00    3200.00  uV
  ...
```

## formats

| format | ext | read | write | notes |
|--------|-----|------|-------|-------|
| EDF | `.edf` | ✓ | ✓ | 16-bit samples, byte-identical round-trip |
| BDF | `.bdf` | ✓ | ✓ | 24-bit samples (biosemi), 0xFF BIOSEMI header |
| MAT | `.mat` | ✓ | ✓ | v7 via matrw, stores channels×samples double matrix |

format detection works by extension first, then magic bytes as fallback.
so if you've got a file with a weird extension it'll still figure it out

### mat layout

when writing MAT files, the data is stored as:
- `data` - channels × samples double matrix
- `labels` - cell array of channel name strings
- `srate` - scalar sample rate
- `patient`, `recording` - optional metadata strings

### what gets lost in conversion

- EDF/BDF → MAT: per-channel sample rates collapse to a single rate, physical
  units are not preserved, start time is lost
- MAT → EDF/BDF: digital min/max defaults to 16-bit range, transducer and
  prefiltering info is empty
- EDF ↔ BDF: lossless (just different sample bit depths)

## building

requires nix for now (or just a rust toolchain):

```bash
nix develop    # enter devshell
cargo build
cargo test
```

to run the full test suite including real-world test files:

```bash
./testdata/fetch.sh          # download test files from teuniz.net
cargo test -- --include-ignored
```

## licence

MIT
