use std::time::{Duration, Instant};

use aes_gcm_siv::{
    AeadCore, Aes128GcmSiv, KeyInit, Nonce,
    aead::{Aead, OsRng},
};
use anyhow::Result;
use args::{Action, Args, Encryption};
use clap::Parser;
use dosr::Dosr;
use hound::{WavSpec, WavWriter};
use itertools::Itertools;
use k256::{Secp256k1, SecretKey, elliptic_curve::PublicKey, pkcs8::DecodePublicKey};

mod args;

fn main() {
    let args = Args::parse();
    let duration = Duration::from_millis(args.duration_ms);
    let sample_rate = args.sample_rate;
    let dosr = Dosr::default()
        .with_duration_s(duration.as_secs_f32())
        .with_sample_rate(sample_rate);

    match args.action {
        Action::Encode {
            message,
            output_path,
            encryption_options,
        } => encode(
            &message,
            &output_path,
            &encryption_options,
            &dosr,
            args.verbose,
        ),
        Action::Decode {
            input_path,
            encryption_options,
        } => decode(&input_path, &encryption_options, &dosr, args.verbose),
    }
}

fn encode(
    message: &str,
    output_path: &str,
    encryption_options: &Option<Encryption>,
    dosr: &Dosr,
    verbose: bool,
) {
    let data = message.as_bytes().to_vec();
    let start = Instant::now();
    let data =
        if let Some(cipher) = create_cipher(encryption_options).expect("Failed to create cipher") {
            let nonce = Aes128GcmSiv::generate_nonce(&mut OsRng);
            let encrypted = cipher.encrypt(&nonce, data.as_ref()).unwrap();
            [nonce.to_vec(), encrypted].concat()
        } else {
            data
        };
    let encryption_time = start.elapsed();
    let start = Instant::now();
    let samples = dosr.encode_data(&data);
    let encoding_time = start.elapsed();
    if verbose {
        eprintln!("Encoding time: {:?}", encoding_time);
        eprintln!("Encryption time: {:?}", encryption_time);
    }
    let spec = WavSpec {
        channels: 1,
        sample_rate: dosr.sample_rate() as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = WavWriter::create(output_path, spec).expect("Failed to create output file");
    samples.iter().for_each(|s| {
        writer.write_sample(*s).expect("Failed to write sample");
    });
    writer.finalize().expect("Failed to finalize output file");
}

fn decode(input_path: &str, encryption_options: &Option<Encryption>, dosr: &Dosr, verbose: bool) {
    let samples = hound::WavReader::open(input_path)
        .expect("Failed to open input file")
        .samples()
        .flatten()
        .collect_vec();
    let start = Instant::now();
    let decoded = dosr.decode(&samples);
    let decoding_time = start.elapsed();
    let start = Instant::now();
    let decoded =
        if let Some(cipher) = create_cipher(encryption_options).expect("Failed to create cipher") {
            let nonce = decoded.iter().take(12).cloned().collect_vec();
            let encrypted = decoded.into_iter().skip(12).collect_vec();
            let nonce = Nonce::from_slice(&nonce);
            cipher.decrypt(nonce, encrypted.as_ref()).unwrap()
        } else {
            decoded
        };
    let decryption_time = start.elapsed();
    if verbose {
        eprintln!("Decoding time: {:?}", decoding_time);
        eprintln!("Decryption time: {:?}", decryption_time);
    }
    let decoded = String::from_utf8(decoded).expect("Failed to decode message");
    println!("Decoded message:\n{decoded}");
}

fn create_cipher(encryption_options: &Option<Encryption>) -> Result<Option<Aes128GcmSiv>> {
    let Some(encryption_options) = encryption_options else {
        return Ok(None);
    };

    let key = match encryption_options {
        Encryption::Sym { key_path } => std::fs::read(key_path)?,
        Encryption::Asym {
            private_key_path,
            public_key_path,
        } => {
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
