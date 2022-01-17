use agsol_gold_client::pad_to_32_bytes;
use agsol_gold_contract::instruction::factory::{verify_auction, VerifyAuctionArgs};

use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signer::keypair::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use std::path::PathBuf;
use structopt::StructOpt;

#[rustfmt::skip]
const TEST_ADMIN_SECRET: [u8; 64] = [
    81, 206, 2, 84, 194, 25, 213, 226, 169, 97,
    254, 229, 43, 106, 226, 29, 181, 244, 192, 48,
    232, 94, 249, 178, 120, 15, 117, 219, 147, 151,
    148, 102, 184, 227, 91, 48, 138, 79, 190, 249,
    113, 152, 84, 101, 174, 107, 202, 130, 113, 205,
    134, 62, 149, 92, 86, 216, 113, 95, 245, 151,
    34, 17, 205, 3
];

const MIN_BALANCE: u64 = 1_000_000_000; // lamports

// default option is deploying on the testnet
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
struct Opt {
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet")
    )]
    devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    mainnet: bool,
    #[structopt(
        long,
        short = "-contract-admin",
        help("The contract admin's keypair file (default = test admin)")
    )]
    contract_admin_keypair: Option<PathBuf>,
    #[structopt(long, help("The id of the auction to verify"))]
    auction_id: String,
}

pub fn main() {
    env_logger::init();
    let opt = Opt::from_args();
    let (net, should_airdrop) = if opt.mainnet {
        ("mainnet-beta", false)
    } else if opt.devnet {
        ("devnet", true)
    } else {
        ("testnet", true)
    };
    let connection_url = format!("https://api.{}.solana.com", net);
    let connection = RpcClient::new_with_commitment(connection_url, CommitmentConfig::confirmed());
    // unwraps below are fine because we are working with pre-tested consts
    // or panicking during initializiation is acceptable in this case
    let admin_keypair = if let Some(keypair_path) = opt.contract_admin_keypair {
        read_keypair_file(keypair_path).unwrap()
    } else {
        Keypair::from_bytes(&TEST_ADMIN_SECRET).unwrap()
    };

    if let Err(e) = try_main(&connection, &admin_keypair, should_airdrop, opt.auction_id) {
        error!("{}", e);
    }
}

fn try_main(
    connection: &RpcClient,
    admin_keypair: &Keypair,
    should_airdrop: bool,
    auction_id: String,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let admin_balance = connection.get_balance(&admin_keypair.pubkey())?;
    if admin_balance < MIN_BALANCE {
        warn!(
            "admin balance ({}) is below threshold ({})",
            admin_balance, MIN_BALANCE
        );
        if should_airdrop {
            let airdrop_signature =
                connection.request_airdrop(&admin_keypair.pubkey(), MIN_BALANCE)?;
            let mut i = 0;
            while !connection.confirm_transaction(&airdrop_signature)? {
                i += 1;
                if i >= 100 {
                    break;
                }
            }
        }
    }

    let id_bytes = pad_to_32_bytes(&auction_id)?;

    let verify_args = VerifyAuctionArgs {
        contract_admin_pubkey: admin_keypair.pubkey(),
        auction_id: id_bytes,
    };

    let verify_ix = verify_auction(&verify_args);

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[verify_ix],
        Some(&admin_keypair.pubkey()),
        &[admin_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "Auction {} successfully withdrawn    signature: {:?}",
        auction_id, signature
    );

    Ok(())
}
