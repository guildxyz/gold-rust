mod cli_opts;
mod cli_utils;
mod pool_cache;

use cli_opts::AuctionBotOpt;
use cli_utils::*;

use agsol_gold_contract::utils::{pad_to_32_bytes, unpad_id};

use agsol_gold_contract::instruction::factory::{close_auction_cycle, CloseAuctionCycleArgs};
use agsol_gold_contract::pda::auction_pool_seeds;
use agsol_gold_contract::state::{AuctionPool, TokenConfig, TokenType};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_wasm_client::{Net, RpcClient};

use env_logger::Env;
use log::{error, info, warn};

use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::clock::UnixTimestamp;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

use pool_cache::{ManagedPool, PoolRecord};

const MIN_BALANCE: u64 = 1_000_000_000; // lamports
const SLEEP_DURATION: u64 = 1000; // milliseconds

#[tokio::main]
pub async fn main() {
    let opt = AuctionBotOpt::from_args();
    let (net, should_airdrop) = if opt.mainnet {
        (Net::Mainnet, false)
    } else if opt.devnet {
        (Net::Devnet, true)
    } else if opt.localnet {
        (Net::Localhost, true)
    } else {
        (Net::Testnet, true)
    };
    let mut client = RpcClient::new(net);

    let bot_keypair = parse_keypair(opt.keypair, &TEST_BOT_SECRET);

    let focused_id_bytes = opt
        .auction_id
        .clone()
        .map(|id| pad_to_32_bytes(&id).expect("auction id could not be parsed"));

    if let Some(id_bytes) = focused_id_bytes {
        if !is_existing_auction(&mut client, id_bytes).await {
            panic!(
                "auction {} does not exist in pool.",
                opt.auction_id.unwrap()
            );
        }
    }

    // set default logging level to 'info'
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    // initialize auction pool cache
    let mut managed_pool = ManagedPool::new();

    loop {
        if let Err(e) = try_main(
            &mut client,
            &bot_keypair,
            should_airdrop,
            &mut managed_pool,
            focused_id_bytes,
        )
        .await
        {
            error!("{}", e);
        }
    }
}

async fn try_main(
    client: &mut RpcClient,
    bot_keypair: &Keypair,
    should_airdrop: bool,
    managed_pool: &mut ManagedPool,
    focused_id_bytes: Option<[u8; 32]>,
) -> Result<(), anyhow::Error> {
    // log error auctions
    if !managed_pool.error_auctions.is_empty() {
        warn!("auctions with error on cycle closing:");
        managed_pool.error_auctions.iter().for_each(|auction_id| {
            println!("{:?}", unpad_id(auction_id));
        });
    }
    // airdrop if necessary
    let bot_balance = client.get_balance(&bot_keypair.pubkey()).await?;
    if bot_balance < MIN_BALANCE {
        warn!(
            "bot balance ({}) is below threshold ({})",
            bot_balance, MIN_BALANCE
        );
        if should_airdrop {
            let blockhash = client.get_latest_blockhash().await?;
            client
                .request_airdrop(&bot_keypair.pubkey(), MIN_BALANCE, &blockhash)
                .await?;
        }
    }
    // get current blockchain time
    let slot = client.get_slot().await?;
    let block_time = client.get_block_time(slot).await?;
    info!("time: {} [s]", block_time);

    // close cycle(s)
    if let Some(id_bytes) = focused_id_bytes {
        try_close_cycle(client, id_bytes, bot_keypair, block_time, managed_pool).await?;
    } else {
        // read auction pool
        let (auction_pool_pubkey, _) =
            Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);
        let auction_pool_data = client.get_account_data(&auction_pool_pubkey).await?;
        let auction_pool: AuctionPool = try_from_slice_unchecked(&auction_pool_data)?;
        // check pool load
        let load = auction_pool.pool.len() as f64 / auction_pool.max_len as f64;
        if load > 0.8 {
            warn!(
                "auction pool is {}% full. Consider allocating additional data.",
                load
            );
        }
        // read individual states
        for auction_id_bytes in auction_pool.pool.iter() {
            try_close_cycle(
                client,
                *auction_id_bytes,
                bot_keypair,
                block_time,
                managed_pool,
            )
            .await?;
        }
    }

    // sleep between iterations
    std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION));

    Ok(())
}

/// Tries to send a close cycle transaction on the currently processed auction.
///
/// It sends the transaction only if the auction is active and the current cycle finished.
async fn try_close_cycle(
    client: &mut RpcClient,
    auction_id: [u8; 32],
    bot_keypair: &Keypair,
    block_time: UnixTimestamp,
    managed_pool: &mut ManagedPool,
) -> Result<(), anyhow::Error> {
    // fetch from pool cache or insert if new auction
    let pool_record = if let Some(record) = managed_pool
        .get_or_insert_auction(client, auction_id, block_time)
        .await?
    {
        record
    } else {
        return Ok(());
    };

    if let Err(err) = close_cycle(client, &auction_id, pool_record, bot_keypair).await {
        // report error on the pool cache
        let is_unexpected_error = pool_record.report_error(client, &err).await?;

        if is_unexpected_error {
            if pool_record.is_faulty_auction() {
                managed_pool.error_auctions.insert(auction_id);
            }

            error!(
                "auction \"{}\"\n{:?}",
                String::from_utf8_lossy(&auction_id),
                err
            );
        }
    } else {
        // update pool record cache on success
        pool_record.update_cycle_state(client).await?;
        pool_record.reset_error_streak();
    }

    Ok(())
}

/// Constructs close cycle arguments and sends the transaction.
///
/// Returns error only if the transaction call failed.
async fn close_cycle(
    client: &mut RpcClient,
    auction_id: &[u8; 32],
    pool_record: &mut PoolRecord,
    bot_keypair: &Keypair,
) -> Result<(), anyhow::Error> {
    let token_type = match pool_record.root_state.token_config {
        TokenConfig::Nft(_) => TokenType::Nft,
        TokenConfig::Token(_) => TokenType::Token,
    };

    let top_bidder = if pool_record.cycle_state.bid_history.is_empty() {
        None
    } else {
        pool_record
            .cycle_state
            .bid_history
            .get_last_element()
            .map(|x| x.bidder_pubkey)
    };
    let close_auction_cycle_args = CloseAuctionCycleArgs {
        payer_pubkey: bot_keypair.pubkey(),
        auction_owner_pubkey: pool_record.root_state.auction_owner,
        top_bidder_pubkey: top_bidder,
        auction_id: *auction_id,
        next_cycle_num: pool_record.root_state.status.current_auction_cycle,
        token_type,
    };
    let close_auction_cycle_ix = close_auction_cycle(&close_auction_cycle_args);

    let latest_blockhash = client.get_latest_blockhash().await?;

    let transaction = Transaction::new_signed_with_payer(
        &[close_auction_cycle_ix],
        Some(&bot_keypair.pubkey()),
        &[bot_keypair],
        latest_blockhash,
    );

    let signature = client.send_transaction(&transaction).await?;
    info!(
        "auction \"{}\"\nclosed cycle: {}\nsignature: {:?}",
        String::from_utf8_lossy(auction_id),
        pool_record.root_state.status.current_auction_cycle,
        signature
    );

    Ok(())
}

async fn is_existing_auction(client: &mut RpcClient, focused_id_bytes: [u8; 32]) -> bool {
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);
    let auction_pool_data = client.get_account_data(&auction_pool_pubkey).await.unwrap();
    let auction_pool: AuctionPool = try_from_slice_unchecked(&auction_pool_data).unwrap();

    auction_pool.pool.binary_search(&focused_id_bytes).is_ok()
}
