use std::time::{Duration, Instant};

use dosr::MfskConfig;
use hound::{WavSpec, WavWriter};

fn main() {
    let msg = std::env::args().nth(1).unwrap_or("Hello world".to_owned());
    let duration = std::env::args()
        .nth(2)
        .and_then(|arg| arg.parse().ok())
        .unwrap_or(100);

    let duration = Duration::from_millis(duration);
    let sample_rate = 44100.0;

    let config = MfskConfig::new(4, 6, duration.as_secs_f32(), sample_rate);

    let start = Instant::now();
    let samples = config.encode_data(msg.as_bytes());
    let elapsed = start.elapsed();
    eprintln!("Encoding time: {:?}", elapsed);

    let freqs = config.decode(&samples);
    eprintln!("Encoded freqs: {:#4.2?}", freqs);

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
