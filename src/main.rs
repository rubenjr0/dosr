use std::time::{Duration, Instant};

use aes_gcm_siv::{Aes128GcmSiv, KeyInit};
use argh::FromArgs;
use dosr::Dosr;
use hound::{WavSpec, WavWriter};
use itertools::Itertools;

#[derive(FromArgs)]
/// Arguments for DOSR
struct Args {
    /// message to encode
    #[argh(positional)]
    message: String,

    /// duration of each symbol in milliseconds
    #[argh(option, default = "100")]
    duration_ms: u64,

    /// sample rate in Hz
    #[argh(option, default = "44100.0")]
    sample_rate: f32,

    /// key path
    #[argh(option, short = 'k')]
    key_path: Option<String>,

    /// perform encoding
    #[argh(switch, short = 'e')]
    encode: bool,

    /// output file path
    #[argh(option, short = 'o')]
    output_path: Option<String>,

    /// perform decoding
    #[argh(switch, short = 'd')]
    decode: bool,

    /// input file path
    #[argh(option, short = 'i')]
    input_path: Option<String>,

    /// verbose
    #[argh(switch, short = 'v')]
    verbose: bool,
}

fn main() {
    let args: Args = argh::from_env();
    let duration = Duration::from_millis(args.duration_ms);
    let sample_rate = args.sample_rate;
    let dosr = Dosr::new(4, 6, duration.as_secs_f32(), sample_rate);

    let data = args.message.as_bytes();
    let cipher = if let Some(key_path) = args.key_path {
        let key_bytes = std::fs::read(&key_path).expect("Failed to read key file");
        let cipher = Aes128GcmSiv::new_from_slice(&key_bytes).expect("Failed to create cipher");
        Some(cipher)
    } else {
        None
    };
    if !(args.encode || args.decode) {
        panic!("No action specified");
    }
    let samples = if args.encode {
        encode(data, &dosr, &cipher, args.verbose)
    } else {
        hound::WavReader::open(args.input_path.as_ref().expect("Input path is required"))
            .expect("Failed to open input file")
            .samples()
            .flatten()
            .collect_vec()
    };
    if let Some(path) = args.output_path {
        let spec = WavSpec {
            channels: 1,
            sample_rate: dosr.sample_rate() as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };

        let mut writer = WavWriter::create(path, spec).expect("Failed to create output file");
        samples.iter().for_each(|s| {
            writer.write_sample(*s).expect("Failed to write sample");
        });
        writer.finalize().expect("Failed to finalize output file");
    }
    if args.decode {
        let decoded = decode(&samples, &dosr, &cipher, args.verbose);
        println!("{decoded}");
    }
}

fn encode(data: &[u8], dosr: &Dosr, cipher: &Option<Aes128GcmSiv>, verbose: bool) -> Vec<f32> {
    let start = Instant::now();
    let samples = dosr.encode_data(data, cipher);
    let elapsed = start.elapsed();
    if verbose {
        eprintln!("Encoding time: {:?}", elapsed);
    }
    samples
}

fn decode(samples: &[f32], dosr: &Dosr, cipher: &Option<Aes128GcmSiv>, verbose: bool) -> String {
    let start = Instant::now();
    let decoded = dosr.decode(samples, cipher);
    let elapsed = start.elapsed();
    if verbose {
        eprintln!("Decoding time: {:?}", elapsed);
    }
    String::from_utf8(decoded).expect("Failed to decode message")
}
