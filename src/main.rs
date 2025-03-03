use std::time::{Duration, Instant};

use aes_gcm_siv::{Aes128GcmSiv, KeyInit};
use anyhow::Result;
use clap::{Parser, Subcommand};
use dosr::Dosr;
use hound::{WavSpec, WavWriter};
use itertools::Itertools;
use k256::{Secp256k1, SecretKey, elliptic_curve::PublicKey, pkcs8::DecodePublicKey};

#[derive(Subcommand)]
enum Action {
    Encode(EncodeArgs),
    Decode(DecodeArgs),
}

#[derive(Parser)]
/// Encode a message using DOSR
struct EncodeArgs {
    /// message to encode
    message: String,

    /// output file path
    output_path: String,

    /// encryption method: symmetric, asymmetric
    #[command(subcommand)]
    encryption: Option<Encryption>,
}

#[derive(Parser)]
/// Decode a message using DOSR
struct DecodeArgs {
    /// output file path
    input_path: String,

    /// encryption method: symmetric, asymmetric
    #[command(subcommand)]
    encryption: Option<Encryption>,
}

#[derive(Subcommand)]
enum Encryption {
    Symmetric(SymmetricKey),
    Asymmetric(AsymmetricKey),
}

#[derive(Parser)]
/// Arguments for symmetric encryption
struct SymmetricKey {
    /// path to the key file
    key_path: String,
}

#[derive(Parser)]
/// Arguments for asymmetric encryption
struct AsymmetricKey {
    /// path to the private key der file
    private_key_path: String,

    /// path to the public key der file
    public_key_path: String,
}

#[derive(Parser)]
/// Arguments for DOSR
struct Args {
    /// duration of each symbol in milliseconds
    #[clap(short, default_value = "100")]
    duration_ms: u64,

    /// sample rate in Hz
    #[clap(long, default_value = "44100.0")]
    sample_rate: f32,

    /// action to perform: encode, decode
    #[command(subcommand)]
    action: Action,

    /// do not display timing information
    #[clap(short, action = clap::ArgAction::SetFalse)]
    silent: bool,
}

fn main() {
    let args = Args::parse();
    let duration = Duration::from_millis(args.duration_ms);
    let sample_rate = args.sample_rate;
    let dosr = Dosr::new(4, 6, duration.as_secs_f32(), sample_rate);

    match args.action {
        Action::Encode(encode_args) => encode(&encode_args, &dosr, args.silent),
        Action::Decode(decode_args) => decode(&decode_args, &dosr, args.silent),
    }
}

fn encode(args: &EncodeArgs, dosr: &Dosr, silent: bool) {
    let data = args.message.as_bytes();
    let cipher = create_cipher(&args.encryption).expect("Failed to create cipher");
    let start = Instant::now();
    let samples = dosr.encode_data(data, &cipher);
    let elapsed = start.elapsed();
    if !silent {
        eprintln!("Encoding time: {:?}", elapsed);
    }
    let spec = WavSpec {
        channels: 1,
        sample_rate: dosr.sample_rate() as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer =
        WavWriter::create(&args.output_path, spec).expect("Failed to create output file");
    samples.iter().for_each(|s| {
        writer.write_sample(*s).expect("Failed to write sample");
    });
    writer.finalize().expect("Failed to finalize output file");
}

fn decode(args: &DecodeArgs, dosr: &Dosr, silent: bool) {
    let samples = hound::WavReader::open(&args.input_path)
        .expect("Failed to open input file")
        .samples()
        .flatten()
        .collect_vec();
    let cipher = create_cipher(&args.encryption).expect("Failed to create cipher");
    let start = Instant::now();
    let decoded = dosr.decode(&samples, &cipher);
    let elapsed = start.elapsed();
    if !silent {
        eprintln!("Decoding time: {:?}", elapsed);
    }
    let decoded = String::from_utf8(decoded).expect("Failed to decode message");
    println!("{decoded}");
}

fn create_cipher(encryption_options: &Option<Encryption>) -> Result<Option<Aes128GcmSiv>> {
    let Some(encryption_options) = encryption_options else {
        return Ok(None);
    };

    let key = match encryption_options {
        Encryption::Symmetric(SymmetricKey { key_path }) => std::fs::read(key_path)?,
        Encryption::Asymmetric(AsymmetricKey {
            private_key_path,
            public_key_path,
        }) => {
            let private_key_bytes = std::fs::read(private_key_path)?;
            let private_key = SecretKey::from_sec1_der(&private_key_bytes)?;
            let public_key = PublicKey::<Secp256k1>::read_public_key_der_file(public_key_path)?;
            let secret =
                k256::ecdh::diffie_hellman(private_key.to_nonzero_scalar(), public_key.as_affine());
            let mut key = vec![0u8; 16];
            secret
                .extract::<k256::sha2::Sha256>(None)
                .expand(&[], &mut key)
                .map_err(|err| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to expand key: {}", err),
                    )
                })?;
            key
        }
    };

    let cipher = Aes128GcmSiv::new_from_slice(&key)
        .expect("Failed to create cipher, the key should be 16 bytes long");
    Ok(Some(cipher))
}
