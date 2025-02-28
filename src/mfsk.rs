use std::f32;

use bitvec::{order::Msb0, view::BitView};
use itertools::Itertools;
use rustfft::{FftPlanner, num_complex::Complex};

use crate::{DF, F0};

type Chunk = u8;
type Frequency = f32;
type Sample = f32;

/// A vector of chunks representing a frame of data.
type Frame = Vec<Chunk>;
/// A vector of samples representing an encoded frame.
type RawFrame = Vec<Sample>;

#[derive(Debug)]
pub struct MfskConfig {
    /// Base frequency (Hz)
    base_freq: f32,
    /// Frequency delta (Hz)
    delta_freq: f32,
    bits_per_chunk: usize,
    values_per_chunk: usize,
    /// Number of chunks transmitted simultaneously
    chunks_per_frame: usize,
    /// Sample rate (Hz)
    sample_rate: f32,
    /// Duration of each audio frame (seconds)
    duration_s: f32,
}

impl MfskConfig {
    pub fn new(
        bits_per_chunk: usize,
        chunks_per_frame: usize,
        duration_s: f32,
        sample_rate: f32,
    ) -> Self {
        Self {
            base_freq: F0,
            delta_freq: DF,
            chunks_per_frame,
            bits_per_chunk,
            values_per_chunk: 2usize.pow(bits_per_chunk as u32),
            duration_s,
            sample_rate,
        }
    }
}

/// Encoding functionality
impl MfskConfig {
    pub fn calculate_frequency(&self, data: u8, chunk_index: usize) -> f32 {
        assert!(data < self.values_per_chunk as u8, "Value exceeds maximum");
        assert!(
            chunk_index < self.chunks_per_frame,
            "Chunk index out of bounds"
        );
        self.base_freq
            + (data + (self.values_per_chunk * chunk_index) as u8) as f32 * self.delta_freq
    }

    /// Generates samples for a sine wave with the specified arguments
    fn generate_sine_wave(&self, frequency: f32, amplitude: f32) -> Vec<f32> {
        let num_samples = (self.duration_s * self.sample_rate) as u32;
        (0..num_samples)
            .map(|n| {
                let time = n as f32 / self.sample_rate;
                amplitude * (2.0 * f32::consts::PI * frequency * time).sin()
            })
            .collect()
    }

    fn bytes_to_chunks(&self, data: &[u8]) -> Vec<Chunk> {
        let bit_view = data.view_bits::<Msb0>();
        bit_view
            .chunks(self.bits_per_chunk)
            .map(|c| {
                c.iter()
                    .fold(0u8, |acc, bit| (acc << 1) | if *bit { 1 } else { 0 })
            })
            .collect_vec()
    }

    fn chunks_to_frames(&self, chunks: &[Chunk]) -> Vec<Frame> {
        chunks
            .chunks(self.chunks_per_frame)
            .map(|chunk| chunk.to_vec())
            .collect_vec()
    }

    fn encode_frame(&self, frame: Frame) -> RawFrame {
        let num_samples = (self.duration_s * self.sample_rate) as usize;
        let mut samples = vec![0.0; num_samples];
        frame
            .into_iter()
            .enumerate()
            .map(|(chunk_idx, v)| self.calculate_frequency(v, chunk_idx))
            .map(|f| self.generate_sine_wave(f, 0.5))
            .for_each(|w| {
                for i in 0..num_samples {
                    samples[i] += w[i];
                }
            });
        samples
    }

    pub fn encode_data(&self, data: &[u8]) -> Vec<f32> {
        let chunks = self.bytes_to_chunks(data);
        let frames = self.chunks_to_frames(&chunks);
        frames
            .into_iter()
            .flat_map(|frame| self.encode_frame(frame))
            .collect_vec()
    }
}

/// Decoding functionality
impl MfskConfig {
    fn split_into_frames(&self, samples: &[f32]) -> Vec<RawFrame> {
        let samples_per_frame = (self.sample_rate * self.duration_s) as usize;
        samples
            .chunks(samples_per_frame)
            .map(|chunk| chunk.to_vec())
            .collect_vec()
    }

    fn perform_fft(&self, encoded_frame: &[f32]) -> Vec<Complex<f32>> {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(encoded_frame.len());
        let mut buffer = encoded_frame
            .iter()
            .map(|s| Complex::new(*s, 0.0))
            .collect_vec();
        fft.process(&mut buffer);
        buffer
    }

    fn normalize_fft(&self, fft_output: &[Complex<f32>]) -> Vec<f32> {
        let magnitudes = fft_output
            .iter()
            .take(fft_output.len() / 2)
            .map(|c| c.norm())
            .collect_vec();
        let max_magnitude = magnitudes.iter().cloned().fold(0.0f32, f32::max);
        magnitudes.iter().map(|m| m / max_magnitude).collect_vec()
    }

    fn detect_frequencies(&self, samples: &[f32]) -> Vec<Frequency> {
        let fft_output = self.perform_fft(samples);
        let magnitudes = self.normalize_fft(&fft_output);
        let bin_width = self.sample_rate / fft_output.len() as f32;
        let mut frequencies = vec![];
        for i in 0..magnitudes.len() {
            let mag = magnitudes[i];
            if mag > 0.4 && mag > magnitudes[i - 1] && mag > magnitudes[i + 1] {
                frequencies.push(i as f32 * bin_width);
            }
        }
        frequencies
    }

    fn decode_frequency(&self, freq: f32, chunk_index: usize) -> u8 {
        let value = ((freq - self.base_freq) / self.delta_freq).round() as usize;
        let value = value - self.values_per_chunk * chunk_index;
        value as u8
    }

    /// Decodes a vector of frequencies into a frame.
    fn decode_frame(&self, samples: &[f32]) -> Frame {
        self.detect_frequencies(samples)
            .iter()
            .enumerate()
            .map(|(chunk_idx, f)| self.decode_frequency(*f, chunk_idx))
            .collect_vec()
    }

    pub fn decode(&self, samples: &[f32]) -> Vec<u8> {
        self.split_into_frames(samples)
            .iter()
            .flat_map(|frame| self.decode_frame(frame))
            .chunks(8 / self.bits_per_chunk)
            .into_iter()
            .map(|c| c.fold(0u8, |acc, x| (acc << self.bits_per_chunk) | (x)))
            .collect_vec()
    }
}
