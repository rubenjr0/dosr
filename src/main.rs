use std::time::{Duration, Instant};

use aes_gcm_siv::Aes128GcmSiv;
use argh::FromArgs;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, aead::OsRng};
use dosr::Dosr;
use hound::{WavSpec, WavWriter};

#[derive(FromArgs)]
/// Arguments for DOSR
struct Args {
    /// message to encode
    #[argh(positional)]
    message: String,

    /// duration of each symbol in milliseconds
    #[argh(option, short = 'd', default = "100")]
    duration_ms: u64,

    /// sample rate in Hz
    #[argh(option, short = 's', default = "44100.0")]
    sample_rate: f32,

    /// verbose
    #[argh(switch, short = 'v')]
    verbose: bool,
}

fn main() {
    let args: Args = argh::from_env();
    let duration = Duration::from_millis(args.duration_ms);
    let sample_rate = args.sample_rate;
    let config = Dosr::new(4, 6, duration.as_secs_f32(), sample_rate);

    let data = args.message.as_bytes();
    let start = Instant::now();
    let key = Aes128GcmSiv::generate_key(&mut OsRng);
    let samples = config.encode_data(data, &key);
    let elapsed = start.elapsed();
    if args.verbose {
        eprintln!("Encoding time: {:?}", elapsed);
    }

    let dec = config.decode(&samples, &key);
    let dec = String::from_utf8(dec).unwrap();

    eprintln!("{}", dec);

    let spec = WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = WavWriter::create("msg.wav", spec).unwrap();
    samples.iter().for_each(|s| {
        writer.write_sample(*s).unwrap();
    });
    writer.finalize().unwrap();
}
