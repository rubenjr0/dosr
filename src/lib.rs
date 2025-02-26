use std::{f32, sync::Arc, time::Duration};

use hound::WavSpec;
use itertools::Itertools;
use rodio::{OutputStream, Sink, Source, source::SineWave};
use rustfft::{Fft, num_complex::Complex};

const F0: f32 = 1875.0;
const DF: f32 = 46.875;

fn encode_freq(data: u8) -> f32 {
    F0 + data as f32 * DF
}

fn decode_freq(freq: f32) -> u8 {
    ((freq - F0) / DF) as u8
}

pub struct Dosr {
    sample_rate: f32,
    duration: Duration,
    samples_per_frame: usize,
    spec: WavSpec,
    fft: Arc<dyn Fft<f32>>,
}

impl Dosr {
    pub fn new(sample_rate: f32, duration: Duration) -> Self {
        let samples_per_frame = (sample_rate * duration.as_secs_f32()) as usize;
        let mut planner = rustfft::FftPlanner::<f32>::new();

        Self {
            sample_rate,
            duration,
            samples_per_frame,
            spec: hound::WavSpec {
                channels: 1,
                sample_rate: sample_rate as u32,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
            fft: planner.plan_fft_forward(samples_per_frame),
        }
    }

    pub fn encode_message(&self, msg: &str) -> Vec<f32> {
        msg.bytes()
            .flat_map(|b| [(b >> 4) & 0b1111, b & 0b1111])
            .map(encode_freq)
            .collect_vec()
    }

    pub fn generate_samples(&self, freqs: &[f32]) -> Vec<f32> {
        freqs
            .iter()
            .flat_map(|f| self.generate_sine_wave(*f, 1.0))
            .collect_vec()
    }

    fn generate_sine_wave(&self, frequency: f32, amplitude: f32) -> Vec<f32> {
        let num_samples = (self.duration.as_secs_f32() * self.sample_rate) as u32;
        (0..num_samples)
            .map(|n| {
                let time = n as f32 / self.sample_rate;
                amplitude * (2.0 * f32::consts::PI * frequency * time).sin()
            })
            .collect()
    }

    pub fn save_samples(&self, samples: &[f32], path: &str) {
        let mut wav = hound::WavWriter::create(path, self.spec).expect("Could not create wav file");
        samples
            .iter()
            .for_each(|s| wav.write_sample(*s).expect("Failed to write sample"));
        wav.finalize().expect("Failed to save wav file");
    }

    pub fn play_message(&self, msg: &str) {
        let (_stream, stream_handle) =
            OutputStream::try_default().expect("Could not open output stream");
        let sink = Sink::try_new(&stream_handle).expect("Could not create sink");
        self.encode_message(msg)
            .into_iter()
            .map(|f| SineWave::new(f).amplify(0.2).take_duration(self.duration))
            .for_each(|s| {
                sink.append(s);
            });
        sink.sleep_until_end();
    }

    pub fn decode_message(&self, samples: &[f32]) -> String {
        let freqs = self.decode_samples(samples);
        let buffer = freqs
            .iter()
            .map(|f: &f32| decode_freq(*f))
            .tuples()
            .map(|(a, b)| (a << 4) | b)
            .collect();
        String::from_utf8(buffer).expect("Could not convert buffer to string")
    }

    pub fn decode_samples(&self, samples: &[f32]) -> Vec<f32> {
        samples
            .chunks(self.samples_per_frame)
            .map(|c| self.perform_fft(c))
            .map(|fft| self.detect_dominant_frequency(&fft))
            .collect_vec()
    }

    fn perform_fft(&self, samples: &[f32]) -> Vec<Complex<f32>> {
        let mut buff: Vec<_> = samples.iter().map(|&s| Complex::new(s, 0.0)).collect();
        self.fft.process(&mut buff);
        buff
    }

    fn detect_dominant_frequency(&self, fft_output: &[Complex<f32>]) -> f32 {
        let (max_index, _) = fft_output
            .iter()
            .take(self.samples_per_frame / 2)
            .enumerate()
            .max_by(|(_, a), (_, b)| a.norm().partial_cmp(&b.norm()).unwrap())
            .unwrap();
        (max_index + 1) as f32 * self.sample_rate / self.samples_per_frame as f32
    }
}
