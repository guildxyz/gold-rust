use solana_client::rpc_client::RpcClient;
use solana_sdk::signer::keypair::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use std::path::PathBuf;

#[rustfmt::skip]
pub const TEST_ADMIN_SECRET: [u8; 64] = [
    81, 206, 2, 84, 194, 25, 213, 226, 169, 97,
    254, 229, 43, 106, 226, 29, 181, 244, 192, 48,
    232, 94, 249, 178, 120, 15, 117, 219, 147, 151,
    148, 102, 184, 227, 91, 48, 138, 79, 190, 249,
    113, 152, 84, 101, 174, 107, 202, 130, 113, 205,
    134, 62, 149, 92, 86, 216, 113, 95, 245, 151,
    34, 17, 205, 3
];

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

pub const MIN_BALANCE: u64 = 1_000_000_000; // lamports

pub fn request_airdrop(connection: &RpcClient, keypair: &Keypair) -> Result<(), anyhow::Error> {
    let airdrop_signature = connection.request_airdrop(&keypair.pubkey(), MIN_BALANCE)?;
    let mut i = 0;
    while !connection.confirm_transaction(&airdrop_signature)? {
        i += 1;
        if i >= 100 {
            break;
        }
    }
    Ok(())
}