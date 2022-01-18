use agsol_gold_admin_panel::{
    parse_keypair, request_airdrop, AdminWithdrawOpt, MIN_BALANCE, TEST_ADMIN_SECRET,
};

use agsol_gold_contract::instruction::factory::{admin_withdraw, AdminWithdrawArgs};

use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

pub fn main() {
    env_logger::init();
    let opt = AdminWithdrawOpt::from_args();

    let (connection_url, should_airdrop) = if opt.mainnet {
        ("https://api.mainnet-beta.solana.com".to_owned(), false)
    } else if opt.devnet {
        ("https://api.devnet.solana.com".to_owned(), true)
    } else if opt.localnet {
        ("http://localhost:8899".to_owned(), true)
    } else {
        ("https://api.testnet.solana.com".to_owned(), true)
    };

    let connection = RpcClient::new_with_commitment(connection_url, CommitmentConfig::confirmed());

    let withdraw_authority_keypair =
        parse_keypair(opt.withdraw_authority_keypair, &TEST_ADMIN_SECRET);

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
            "withdraw authority balance ({}) is below threshold ({})",
            admin_balance, MIN_BALANCE
        );
        if should_airdrop {
            request_airdrop(connection, withdraw_authority_keypair)?;
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
