use agsol_gold_admin_panel::{
    parse_keypair, request_airdrop, DeleteAuctionsOpt, MIN_BALANCE, TEST_ADMIN_SECRET,
};

use agsol_gold_client::pad_to_32_bytes;
use agsol_gold_contract::instruction::factory::{delete_auction, DeleteAuctionArgs};
use agsol_gold_contract::pda::{auction_pool_seeds, auction_root_state_seeds};
use agsol_gold_contract::state::{AuctionPool, AuctionRootState};
use agsol_gold_contract::ID as GOLD_ID;
use agsol_gold_contract::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL;

use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::borsh::try_from_slice_unchecked;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

const SLEEP_DURATION: u64 = 1000; // milliseconds

pub fn main() {
    env_logger::init();
    let opt = DeleteAuctionsOpt::from_args();

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
    auction_id: Option<String>,
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
    // READ AUCTION POOL
    let (auction_pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);
    let auction_pool_data = connection.get_account_data(&auction_pool_pubkey)?;
    let auction_pool: AuctionPool = try_from_slice_unchecked(&auction_pool_data)?;

    // READ INDIVIDUAL STATES
    if let Some(id) = auction_id {
        let id_bytes = pad_to_32_bytes(&id)?;
        let (state_pubkey, _) =
            Pubkey::find_program_address(&auction_root_state_seeds(&id_bytes), &GOLD_ID);
        if let Err(err) = delete_frozen_auction(connection, &id_bytes, &state_pubkey, admin_keypair)
        {
            error!(
                "auction \"{}\" threw error {:?}",
                String::from_utf8_lossy(&id_bytes),
                err
            );
        }
    } else {
        for auction_id in auction_pool.pool.iter() {
            let (state_pubkey, _) =
                Pubkey::find_program_address(&auction_root_state_seeds(auction_id), &GOLD_ID);
            if let Err(err) =
                delete_frozen_auction(connection, auction_id, &state_pubkey, admin_keypair)
            {
                error!(
                    "auction \"{}\" threw error {:?}",
                    String::from_utf8_lossy(auction_id),
                    err
                );
            }
        }
    }

    Ok(())
}

fn delete_frozen_auction(
    connection: &RpcClient,
    auction_id: &[u8; 32],
    state_pubkey: &Pubkey,
    admin_keypair: &Keypair,
) -> Result<(), anyhow::Error> {
    let auction_state_data = connection.get_account_data(state_pubkey)?;
    let mut auction_state: AuctionRootState = try_from_slice_unchecked(&auction_state_data)?;
    let auction_id_string = String::from_utf8_lossy(auction_id);
    // NOTE: Expired auctions could be deleted as well but maybe those should
    // be deleted individually after talking to their owners
    // IF NOT FROZEN, CONTINUE ITERATION
    if !auction_state.status.is_frozen {
        info!("auction {} is not frozen", auction_id_string);
        return Ok(());
    }

    info!("auction {} is frozen", auction_id_string);

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

        let latest_blockhash = connection.get_latest_blockhash()?;

        let transaction = Transaction::new_signed_with_payer(
            &[delete_auction_ix],
            Some(&admin_keypair.pubkey()),
            &[admin_keypair],
            latest_blockhash,
        );

        let signature = connection.send_and_confirm_transaction(&transaction)?;
        info!(
            "auction {}    deleted until cycle: {}    signature: {:?}",
            auction_id_string, auction_state.status.current_auction_cycle, signature
        );

        std::thread::sleep(std::time::Duration::from_millis(SLEEP_DURATION));

        // Check if auction is deleted completely
        if let Ok(auction_state_data) = connection.get_account_data(state_pubkey) {
            auction_state = try_from_slice_unchecked(&auction_state_data)?;
        } else {
            finished = true;
        }
    }
    info!("auction {} deleted", auction_id_string);

    Ok(())
}
