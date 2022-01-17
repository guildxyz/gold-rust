use agsol_gold_contract::instruction::factory::{admin_withdraw, AdminWithdrawArgs};

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
        short = "-withdraw-authority",
        help("The withdraw authority's keypair (default = test admin)")
    )]
    withdraw_authority_keypair: Option<PathBuf>,
    #[structopt(long, short = "-a", help("The amount to withdraw from contract bank"))]
    amount: u64,
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
    let withdraw_authority_keypair = if let Some(keypair_path) = opt.withdraw_authority_keypair {
        read_keypair_file(keypair_path).unwrap()
    } else {
        Keypair::from_bytes(&TEST_ADMIN_SECRET).unwrap()
    };

    if let Err(e) = try_main(
        &connection,
        &withdraw_authority_keypair,
        should_airdrop,
        opt.amount,
    ) {
        error!("{}", e);
    }
}

fn try_main(
    connection: &RpcClient,
    withdraw_authority_keypair: &Keypair,
    should_airdrop: bool,
    amount: u64,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let admin_balance = connection.get_balance(&withdraw_authority_keypair.pubkey())?;
    if admin_balance < MIN_BALANCE {
        warn!(
            "admin balance ({}) is below threshold ({})",
            admin_balance, MIN_BALANCE
        );
        if should_airdrop {
            let airdrop_signature =
                connection.request_airdrop(&withdraw_authority_keypair.pubkey(), MIN_BALANCE)?;
            let mut i = 0;
            while !connection.confirm_transaction(&airdrop_signature)? {
                i += 1;
                if i >= 100 {
                    break;
                }
            }
        }
    }

    let withdraw_args = AdminWithdrawArgs {
        withdraw_authority: withdraw_authority_keypair.pubkey(),
        amount,
    };

    let withdraw_ix = admin_withdraw(&withdraw_args);

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[withdraw_ix],
        Some(&withdraw_authority_keypair.pubkey()),
        &[withdraw_authority_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "Contract funds successfully withdrawn    signature: {:?}",
        signature
    );

    Ok(())
}
