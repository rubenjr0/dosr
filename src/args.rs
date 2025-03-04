use clap::{Parser, Subcommand};

#[derive(Subcommand)]
pub enum Action {
    Encode {
        /// message to encode
        message: String,

        /// output file path
        output_path: String,

        /// encryption method: symmetric, asymmetric
        #[command(subcommand)]
        encryption_options: Option<Encryption>,
    },
    Decode {
        /// output file path
        input_path: String,

        /// encryption method: symmetric, asymmetric
        #[command(subcommand)]
        encryption_options: Option<Encryption>,
    },
}

#[derive(Subcommand)]
pub enum Encryption {
    Symmetric {
        /// path to the key file
        key_path: String,
    },
    Asymmetric {
        /// path to the private key der file
        private_key_path: String,

        /// path to the public key der file
        public_key_path: String,
    },
}

#[derive(Parser)]
/// Arguments for DOSR
pub struct Args {
    /// duration of each symbol in milliseconds
    #[clap(short, default_value = "100")]
    pub duration_ms: u64,

    /// sample rate in Hz
    #[clap(long, default_value = "44100.0")]
    pub sample_rate: f32,

    /// action to perform: encode, decode
    #[command(subcommand)]
    pub action: Action,

    /// do not display timing information
    #[clap(short, action = clap::ArgAction::SetFalse)]
    pub silent: bool,
}
