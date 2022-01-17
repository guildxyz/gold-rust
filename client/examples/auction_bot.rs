use agsol_gold_contract::instruction::factory::{close_auction_cycle, CloseAuctionCycleArgs};
use agsol_gold_contract::pda::{
    auction_cycle_state_seeds, auction_pool_seeds, auction_root_state_seeds,
};
use agsol_gold_contract::state::{
    AuctionCycleState, AuctionPool, AuctionRootState, TokenConfig, TokenType,
};
use agsol_gold_contract::ID as GOLD_ID;
use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::clock::UnixTimestamp;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use std::path::PathBuf;
use structopt::StructOpt;

#[rustfmt::skip]
const TEST_BOT_SECRET: [u8; 64] = [
  145, 203,  89,  29, 222, 184, 219, 205,   5,  91, 167,
   87,  77, 216,  87,  50, 224, 181,  43,  89, 184,  19,
  156, 223, 138, 207,  68,  76, 146, 103,  25, 215,  50,
  110, 172, 245, 231, 233,  15, 190, 123, 231,  13,  53,
  181, 240, 122, 168,  89, 178, 129,  58, 109, 184, 163,
   97, 191,  19, 114, 229, 113, 224,  40,  20
];

const MIN_BALANCE: u64 = 1_000_000_000; // lamports
const SLEEP_DURATION: u64 = 5000; // milliseconds

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
    #[structopt(long, help("The auction bot's keypair file"))]
    keypair: Option<PathBuf>,
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
    let bot_keypair = if let Some(keypair_path) = opt.keypair {
        read_keypair_file(keypair_path).unwrap()
    } else {
        Keypair::from_bytes(&TEST_BOT_SECRET).unwrap()
    };

    loop {
        if let Err(e) = try_main(&connection, &bot_keypair, should_airdrop) {
            error!("{}", e);
        }
    }
}

fn try_main(
    connection: &RpcClient,
    bot_keypair: &Keypair,
    should_airdrop: bool,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let bot_balance = connection.get_balance(&bot_keypair.pubkey())?;
    if bot_balance < MIN_BALANCE {
        warn!(
            "bot balance ({}) is below threshold ({})",
            bot_balance, MIN_BALANCE
        );
        if should_airdrop {
            let airdrop_signature =
                connection.request_airdrop(&bot_keypair.pubkey(), MIN_BALANCE)?;
            let mut i = 0;
            while !connection.confirm_transaction(&airdrop_signature)? {
                i += 1;
                if i >= 100 {
                    break;
                }
            }
        }
    }
    // GET CURRENT BLOCKCHAIN TIME
    let slot = connection.get_slot()?;
    let block_time = connection.get_block_time(slot)?;
    info!("time: {} [s]", block_time);
    std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION));
    // READ AUCTION POOL
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);
    let auction_pool_data = connection.get_account_data(&auction_pool_pubkey)?;
    let auction_pool: AuctionPool = try_from_slice_unchecked(&auction_pool_data)?;
    // READ INDIVIDUAL STATES
    for auction_id in auction_pool.pool.iter() {
        let (state_pubkey, _) =
            Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);
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

    Ok(())
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
    // IF FROZEN OR INACTIVE, CONTINUE ITERATION
    if auction_state.status.is_frozen || auction_state.status.is_finished {
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

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!(
        "auction \"{}\"    cycle: {}    signature: {:?}",
        String::from_utf8_lossy(auction_id),
        auction_state.status.current_auction_cycle,
        signature
    );
    Ok(())
}
