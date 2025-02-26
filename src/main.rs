use std::time::Duration;

use dosr::Dosr;

fn main() {
    let msg = std::env::args().nth(1).expect("Message not supplied");
    let duration = std::env::args()
        .nth(2)
        .expect("Duration not provided")
        .parse()
        .expect("Could not parse duration");

    let sample_rate = 48000.0;
    let duration = Duration::from_millis(duration);

    let dosr = Dosr::new(sample_rate, duration);
    let freqs = dosr.encode_message(&msg);
    eprintln!(
        "{} frequencies, {:?} per freq, {}s total",
        freqs.len(),
        duration,
        freqs.len() as f32 * duration.as_secs_f32()
    );
    let samples = dosr.generate_samples(&freqs);
    dosr.save_samples(&samples, "msg.wav");
    let dec = dosr.decode_message(&samples);
    eprintln!("Decoded: {dec}");
}
