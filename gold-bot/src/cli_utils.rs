use solana_sdk::signer::keypair::{read_keypair_file, Keypair};
use std::path::PathBuf;

#[rustfmt::skip]
pub const TEST_BOT_SECRET: [u8; 64] = [
  145, 203,  89,  29, 222, 184, 219, 205,   5,  91, 167,
   87,  77, 216,  87,  50, 224, 181,  43,  89, 184,  19,
  156, 223, 138, 207,  68,  76, 146, 103,  25, 215,  50,
  110, 172, 245, 231, 233,  15, 190, 123, 231,  13,  53,
  181, 240, 122, 168,  89, 178, 129,  58, 109, 184, 163,
   97, 191,  19, 114, 229, 113, 224,  40,  20
];

// unwraps below are fine because we are working with pre-tested consts
// or panicking during initializiation is acceptable in this case
pub fn parse_keypair(keypair: Option<PathBuf>, default: &[u8]) -> Keypair {
    if let Some(keypair_path) = keypair {
        read_keypair_file(keypair_path).unwrap()
    } else {
        Keypair::from_bytes(default).unwrap()
    }
}
