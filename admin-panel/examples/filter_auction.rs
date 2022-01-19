use agsol_gold_admin_panel::{
    parse_keypair, request_airdrop, FilterAuctionOpt, MIN_BALANCE, TEST_ADMIN_SECRET,
};

use agsol_gold_client::pad_to_32_bytes;

use agsol_gold_contract::instruction::factory::filter_auction;
use agsol_gold_contract::pda::auction_root_state_seeds;
use agsol_gold_contract::state::AuctionRootState;
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
    let opt = FilterAuctionOpt::from_args();
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

    let admin_keypair = parse_keypair(opt.keypair, &TEST_ADMIN_SECRET);

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
            request_airdrop(connection, admin_keypair)?;
        }
    }

    let id_bytes = pad_to_32_bytes(&auction_id)?;

    let auction_filter_result = check_auction_state(connection, &id_bytes);
    if let Err(ref err) = auction_filter_result {
        error!("error while filtering auction \"{}\": {}", auction_id, err);
    }

    // unwrap is fine here since the error case is handled beforehand
    let filter_ix = filter_auction(
        admin_keypair.pubkey(),
        id_bytes,
        !auction_filter_result.unwrap(),
    );

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[filter_ix],
        Some(&admin_keypair.pubkey()),
        &[admin_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "Auction {} successfully filtered    signature: {:?}",
        auction_id, signature
    );

    Ok(())
}

fn check_auction_state(connection: &RpcClient, id_bytes: &[u8]) -> Result<bool, anyhow::Error> {
    let (state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(id_bytes), &GOLD_ID);

    let state_data_result = connection.get_account_data(&state_pubkey);

    if state_data_result.is_err() {
        return Err(anyhow!("auction does not exist."));
    }

    // unwrap is fine here since the error case is handled beforehand
    let root_state: AuctionRootState = try_from_slice_unchecked(&state_data_result.unwrap())?;

    Ok(root_state.status.is_filtered)
}
