use agsol_gold_contract::instruction::factory::{delete_auction, DeleteAuctionArgs};
use agsol_gold_contract::pda::get_auction_pool_seeds;
use agsol_gold_contract::state::{
    AuctionPool, AuctionRootState,
};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_gold_contract::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL;
use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
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
const SLEEP_DURATION: u64 = 5000; // milliseconds
const DELETE_SLEEP_DURATION: u64 = 1000; // milliseconds

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
    let admin_keypair = if let Some(keypair_path) = opt.keypair {
        read_keypair_file(keypair_path).unwrap()
    } else {
        Keypair::from_bytes(&TEST_ADMIN_SECRET).unwrap()
    };

    loop {
        if let Err(e) = try_main(&connection, &admin_keypair, &GOLD_ID, should_airdrop) {
            error!("{}", e);
        }
    }
}

fn try_main(
    connection: &RpcClient,
    admin_keypair: &Keypair,
    program_id: &Pubkey,
    should_airdrop: bool,
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
    // GET CURRENT BLOCKCHAIN TIME
    let slot = connection.get_slot()?;
    let block_time = connection.get_block_time(slot)?;
    info!("time: {} [s]", block_time);
    std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION));
    // READ AUCTION POOL
    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), program_id);
    let auction_pool_data = connection.get_account_data(&auction_pool_pubkey)?;
    let auction_pool: AuctionPool = try_from_slice_unchecked(&auction_pool_data)?;
    // READ INDIVIDUAL STATES
    for (auction_id, state_pubkey) in auction_pool.pool.contents().iter() {
        if let Err(err) = delete_frozen_auction(
            connection,
            auction_id,
            state_pubkey,
            admin_keypair,
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

#[allow(deprecated)] // TODO remove this once solana 1.9.0 is out
fn delete_frozen_auction(
    connection: &RpcClient,
    auction_id: &[u8; 32],
    state_pubkey: &Pubkey,
    admin_keypair: &Keypair,
) -> Result<(), anyhow::Error> {
    let auction_state_data = connection.get_account_data(state_pubkey)?;
    let mut auction_state: AuctionRootState = try_from_slice_unchecked(&auction_state_data)?;
    
    // IF NOT FROZEN, CONTINUE ITERATION
    // Note: Expired auctions could be deleted as well
    if !auction_state.status.is_frozen {
        info!("auction {} is not frozen", String::from_utf8_lossy(auction_id));
        return Ok(());
    }

    info!("auction {} is frozen", String::from_utf8_lossy(auction_id));

    
    let mut finished = false;
    while !finished {
        let delete_auction_args = DeleteAuctionArgs {
            contract_admin_pubkey: admin_keypair.pubkey(),
            auction_owner_pubkey: auction_state.auction_owner,
            auction_id: *auction_id,
            current_auction_cycle: auction_state.status.current_auction_cycle,
            num_of_cycles_to_delete: RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
        };

        let delete_auction_ix = delete_auction(&delete_auction_args);

        // TODO use latest blockhash once it's on the devnet
        let (latest_blockhash, _) = connection.get_recent_blockhash()?;

        let transaction = Transaction::new_signed_with_payer(
            &[delete_auction_ix],
            Some(&admin_keypair.pubkey()),
            &[admin_keypair],
            latest_blockhash,
        );

        // TODO use send_and_confirm_transaction once `get_latest_blockhash` is stabilized
        let signature = connection.send_transaction(&transaction)?;
        info!(
            "auction {}    deleted until cycle: {}    signature: {:?}",
            String::from_utf8_lossy(auction_id),
            auction_state.status.current_auction_cycle,
            signature
        );

        std::thread::sleep(std::time::Duration::from_millis(DELETE_SLEEP_DURATION));

        // Check if auction is deleted completely
        if let Ok(auction_state_data) = connection.get_account_data(state_pubkey) {
            auction_state = try_from_slice_unchecked(&auction_state_data)?;
        } else {
            finished = true;
        }
    }
    info!("auction {} deleted", String::from_utf8_lossy(auction_id));
    
    Ok(())
}
