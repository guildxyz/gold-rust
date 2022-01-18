use agsol_gold_admin_panel::{
    parse_keypair, request_airdrop, ReallocatePoolOpt, MIN_BALANCE, TEST_ADMIN_SECRET,
};

use agsol_gold_contract::instruction::factory::{deallocate_pool, reallocate_pool};
use agsol_gold_contract::pda::auction_pool_seeds;
use agsol_gold_contract::state::AuctionPool;
use agsol_gold_contract::ID as GOLD_ID;

use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

use anyhow::anyhow;

pub fn main() {
    env_logger::init();
    let opt = ReallocatePoolOpt::from_args();

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

    let admin_keypair = parse_keypair(opt.contract_admin_keypair, &TEST_ADMIN_SECRET);

    if let Err(e) = try_main(&connection, &admin_keypair, should_airdrop, opt.size) {
        error!("{}", e);
    }
}

fn try_main(
    connection: &RpcClient,
    admin_keypair: &Keypair,
    should_airdrop: bool,
    size: u32,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let admin_balance = connection.get_balance(&admin_keypair.pubkey())?;
    if admin_balance < MIN_BALANCE {
        warn!(
            "admin balance ({}) is below threshold ({})",
            admin_balance, MIN_BALANCE
        );
        if should_airdrop {
            request_airdrop(connection, admin_keypair)?;
        }
    }

    if let Err(err) = check_pool_size(connection, size) {
        error!("error while reallocating auction pool: {}", err);
    }

    let deallocate_ix = deallocate_pool(&admin_keypair.pubkey());

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[deallocate_ix],
        Some(&admin_keypair.pubkey()),
        &[admin_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "Auction pool deallocated successfully    signature: {:?}",
        signature
    );

    let reallocate_ix = reallocate_pool(&admin_keypair.pubkey(), size);

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[reallocate_ix],
        Some(&admin_keypair.pubkey()),
        &[admin_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "Auction pool reallocated successfully    signature: {:?}",
        signature
    );

    Ok(())
}

fn check_pool_size(connection: &RpcClient, size: u32) -> Result<(), anyhow::Error> {
    let (pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);

    let pool_state_data = connection.get_account_data(&pool_pubkey)?;
    let pool_state: AuctionPool = try_from_slice_unchecked(&pool_state_data)?;

    if pool_state.max_len >= size {
        return Err(anyhow!("provided size smaller than current size"));
    }

    Ok(())
}
