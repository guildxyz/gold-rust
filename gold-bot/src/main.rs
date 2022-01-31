mod cli_opts;
mod cli_utils;

use cli_opts::AuctionBotOpt;
use cli_utils::*;

use agsol_gold_client::pad_to_32_bytes;

use agsol_gold_contract::instruction::factory::{close_auction_cycle, CloseAuctionCycleArgs};
use agsol_gold_contract::pda::{
    auction_cycle_state_seeds, auction_pool_seeds, auction_root_state_seeds,
};
use agsol_gold_contract::state::{
    AuctionCycleState, AuctionPool, AuctionRootState, TokenConfig, TokenType,
};
use agsol_gold_contract::ID as GOLD_ID;

use log::{error, info, warn};
use env_logger::Env;

use solana_client::rpc_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::clock::UnixTimestamp;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

const SLEEP_DURATION: u64 = 5000; // milliseconds

pub fn main() {
    let opt = AuctionBotOpt::from_args();
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

    let bot_keypair = parse_keypair(opt.keypair, &TEST_BOT_SECRET);

    let focused_id_bytes = if let Some(id) = opt.auction_id {
        Some(pad_to_32_bytes(&id).unwrap())
    } else {
        None
    };

    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    loop {
        if let Err(e) = try_main(&connection, &bot_keypair, should_airdrop, focused_id_bytes) {
            error!("{}", e);
        }
    }
}

fn try_main(
    connection: &RpcClient,
    bot_keypair: &Keypair,
    should_airdrop: bool,
    focused_id_bytes: Option<[u8; 32]>,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let bot_balance = connection.get_balance(&bot_keypair.pubkey())?;
    if bot_balance < MIN_BALANCE {
        warn!(
            "bot balance ({}) is below threshold ({})",
            bot_balance, MIN_BALANCE
        );
        if should_airdrop {
            request_airdrop(connection, bot_keypair)?;
        }
    }
    // GET CURRENT BLOCKCHAIN TIME
    let slot = connection.get_slot()?;
    let block_time = connection.get_block_time(slot)?;
    info!("time: {} [s]", block_time);
    std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION));
    if let Some(id_bytes) = focused_id_bytes {
        try_close_cycle(connection, &id_bytes, bot_keypair, block_time);
    } else {
        // READ AUCTION POOL
        let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);
        let auction_pool_data = connection.get_account_data(&auction_pool_pubkey)?;
        let auction_pool: AuctionPool = try_from_slice_unchecked(&auction_pool_data)?;
        // CHECK POOL LOAD
        let load = auction_pool.pool.len() as f64 / auction_pool.max_len as f64;
        if load > 0.8 {
            warn!(
                "auction pool is {}% full. Consider allocating additional data.",
                load
            );
        }
        // READ INDIVIDUAL STATES
        for auction_id_bytes in auction_pool.pool.iter() {
            try_close_cycle(connection, auction_id_bytes, bot_keypair, block_time);
        }
    }

    Ok(())
}

fn try_close_cycle(
    connection: &RpcClient,
    auction_id: &[u8; 32],
    bot_keypair: &Keypair,
    block_time: UnixTimestamp,
) {
    let (state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&(*auction_id)), &GOLD_ID);
    if let Err(err) = close_cycle(
        connection,
        auction_id,
        &state_pubkey,
        bot_keypair,
        block_time,
    ) {
        error!(
            "auction \"{}\" threw error {:?}",
            String::from_utf8_lossy(auction_id),
            err
        );
    }
}

fn close_cycle(
    connection: &RpcClient,
    auction_id: &[u8; 32],
    state_pubkey: &Pubkey,
    bot_keypair: &Keypair,
    block_time: UnixTimestamp,
) -> Result<(), anyhow::Error> {
    let auction_state_data = connection.get_account_data(state_pubkey)?;
    let auction_state: AuctionRootState = try_from_slice_unchecked(&auction_state_data)?;
    let current_cycle_bytes = auction_state.status.current_auction_cycle.to_le_bytes();
    // IF FROZEN OR INACTIVE OR FILTERED, CONTINUE ITERATION
    if auction_state.status.is_frozen
        || auction_state.status.is_finished
        || auction_state.status.is_filtered
    {
        return Ok(());
    }

    let (cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(state_pubkey, &current_cycle_bytes),
        &GOLD_ID,
    );
    let current_cycle_data = connection.get_account_data(&cycle_state_pubkey)?;
    let auction_cycle_state: AuctionCycleState = try_from_slice_unchecked(&current_cycle_data)?;

    // IF NOT EXPIRED, CONTINUE ITERATION
    if block_time < auction_cycle_state.end_time {
        return Ok(());
    }

    let token_type = match auction_state.token_config {
        TokenConfig::Nft(_) => TokenType::Nft,
        TokenConfig::Token(_) => TokenType::Token,
    };

    let top_bidder = if auction_cycle_state.bid_history.is_empty() {
        None
    } else {
        auction_cycle_state
            .bid_history
            .get_last_element()
            .map(|x| x.bidder_pubkey)
    };
    let close_auction_cycle_args = CloseAuctionCycleArgs {
        payer_pubkey: bot_keypair.pubkey(),
        auction_owner_pubkey: auction_state.auction_owner,
        top_bidder_pubkey: top_bidder,
        auction_id: *auction_id,
        next_cycle_num: auction_state.status.current_auction_cycle,
        token_type,
    };
    let close_auction_cycle_ix = close_auction_cycle(&close_auction_cycle_args);

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[close_auction_cycle_ix],
        Some(&bot_keypair.pubkey()),
        &[bot_keypair],
        latest_blockhash,
    );

    let signature = connection.send_transaction(&transaction)?;
    info!(
        "auction \"{}\"    cycle: {}    signature: {:?}",
        String::from_utf8_lossy(auction_id),
        auction_state.status.current_auction_cycle,
        signature
    );
    Ok(())
}
