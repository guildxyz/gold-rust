use agsol_gold_admin_panel::{
    parse_keypair, request_airdrop, ReassignWithdrawOpt, MIN_BALANCE, TEST_ADMIN_SECRET,
};

use agsol_gold_contract::instruction::factory::{
    admin_withdraw_reassign, AdminWithdrawReassignArgs,
};

use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

pub fn main() {
    env_logger::init();
    let opt = ReassignWithdrawOpt::from_args();

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

    // TODO: Only pubkey should be enough
    let new_authority_keypair = read_keypair_file(opt.new_withdraw_authority_keypair).unwrap();

    if let Err(e) = try_main(
        &connection,
        &withdraw_authority_keypair,
        should_airdrop,
        &new_authority_keypair.pubkey(),
    ) {
        error!("{}", e);
    }
}

fn try_main(
    connection: &RpcClient,
    withdraw_authority_keypair: &Keypair,
    should_airdrop: bool,
    new_authority_pubkey: &Pubkey,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let admin_balance = connection.get_balance(&withdraw_authority_keypair.pubkey())?;
    if admin_balance < MIN_BALANCE {
        warn!(
            "admin balance ({}) is below threshold ({})",
            admin_balance, MIN_BALANCE
        );
        if should_airdrop {
            request_airdrop(connection, withdraw_authority_keypair)?;
        }
    }

    let reassign_withdraw_args = AdminWithdrawReassignArgs {
        withdraw_authority: withdraw_authority_keypair.pubkey(),
        new_withdraw_authority: *new_authority_pubkey,
    };

    let reassign_withdraw_ix = admin_withdraw_reassign(&reassign_withdraw_args);

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[reassign_withdraw_ix],
        Some(&withdraw_authority_keypair.pubkey()),
        &[withdraw_authority_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "Withdraw authority successfully transferred    signature: {:?}",
        signature
    );

    Ok(())
}
