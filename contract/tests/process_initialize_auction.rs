#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::{initialize_new_auction, TestUser, TRANSACTION_FEE};

use agsol_common::MaxSerializedLen;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::utils::unpuff_metadata;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;

use agsol_token_metadata::ID as META_ID;

use solana_program::program_option::COption;
use solana_program::pubkey::Pubkey;

const AUCTION_CREATION_COST: u64 = 24_116_400 + TRANSACTION_FEE;

// This file includes the following tests:
//
// Valid use cases:
//   - Creating nft auction
//   - (Test for creating token auction in `process_tokens.rs`)
//
// Invalid use cases:
//   - Creating auction with non-ascii charactes in its id
//   - Creating auction with minimum_bid_amount lower than UNIVERSAL_BID_FLOOR
//   - Creating auction with too short cycle period
//   - Creating auction with too long cycle period
//   - Creating auction with negative encore period
//   - Creating auction with too long encore period
//   - Create auction with an id already taken by the same user
//   - Create auction with an id already taken by another user
//   - (Test for trying to initialize an auction with a full pool in `process_reallocate_pool.rs`)

#[tokio::test]
async fn test_process_initialize_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();
    let non_ascii_bytes = "héllóabcdefghijklmnopqrstuvwxy".as_bytes();
    let mut auction_id = [0_u8; 32];
    auction_id.copy_from_slice(non_ascii_bytes);

    let mut auction_config = AuctionConfig {
        cycle_period: 86400,
        encore_period: 30,
        minimum_bid_amount: 50_000_000,
        number_of_cycles: Some(10),
    };

    // Invalid use case
    // Creating auction with non-ascii characters in its id
    let invalid_auction_id_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        invalid_auction_id_error,
        AuctionContractError::AuctionIdNotAscii
    );

    let auction_id = [123_u8; 32];
    // Invalid use case
    // Initialize auction with invalid minimum_bid_amount
    // minimum_bid_amount < UNIVERSAL_BID_FLOOR
    auction_config.minimum_bid_amount = 10_000_000;
    let invalid_min_bid_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        invalid_min_bid_error,
        AuctionContractError::InvalidMinimumBidAmount
    );

    // Invalid use case
    // Creating auction with too short cycle period
    auction_config.cycle_period = 30;
    auction_config.minimum_bid_amount = 50_000_000;

    let invalid_cycle_period_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        invalid_cycle_period_error,
        AuctionContractError::InvalidCyclePeriod
    );

    // Invalid use case
    // Creating auction with too long cycle period
    auction_config.cycle_period = 50_000_000;
    let invalid_cycle_period_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        invalid_cycle_period_error,
        AuctionContractError::InvalidCyclePeriod
    );

    auction_config.cycle_period = 60;

    // Invalid use case
    // Creating auction with negative encore period
    auction_config.encore_period = -1;
    let negative_encore_period_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        negative_encore_period_error,
        AuctionContractError::InvalidEncorePeriod
    );

    // Invalid use case
    // Creating auction with too long encore period
    auction_config.encore_period = auction_config.cycle_period / 2 + 1;
    let negative_encore_period_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        negative_encore_period_error,
        AuctionContractError::InvalidEncorePeriod
    );

    auction_config.encore_period = 0;

    // Create a valid auction
    let balance_change = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, AUCTION_CREATION_COST);

    // check mint account
    let (master_mint_pubkey, _) =
        Pubkey::find_program_address(&master_mint_seeds(&auction_id), &CONTRACT_ID);
    let (master_edition_pubkey, _) = Pubkey::find_program_address(
        &edition_seeds(&master_mint_pubkey),
        &agsol_token_metadata::ID,
    );

    let master_mint_data = testbench
        .get_mint_account(&master_mint_pubkey)
        .await
        .unwrap();

    assert!(master_mint_data.is_initialized);
    assert_eq!(
        master_mint_data.mint_authority,
        COption::Some(master_edition_pubkey)
    );
    assert_eq!(master_mint_data.supply, 1);
    assert_eq!(master_mint_data.decimals, 0);

    // check holding account
    let (master_holding_pubkey, _) =
        Pubkey::find_program_address(&master_holding_seeds(&auction_id), &CONTRACT_ID);
    let master_holding_data = testbench
        .get_token_account(&master_holding_pubkey)
        .await
        .unwrap();

    assert_eq!(master_holding_data.amount, 1);

    // check metadata
    let (master_metadata_pubkey, _) =
        Pubkey::find_program_address(&metadata_seeds(&master_mint_pubkey), &META_ID);
    let mut master_metadata = testbench
        .get_and_deserialize_account_data::<agsol_token_metadata::state::Metadata>(
            &master_metadata_pubkey,
        )
        .await
        .unwrap();
    unpuff_metadata(&mut master_metadata.data);
    assert_eq!(master_metadata.data.uri, "uri/1.json");

    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let cycle_number_bytes = 1_u64.to_le_bytes();
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &cycle_number_bytes),
        &CONTRACT_ID,
    );

    // Assert length of the root state data
    let auction_root_state_data = testbench
        .get_account_data(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(
        auction_root_state_data.len(),
        AuctionRootState::MAX_SERIALIZED_LEN + agsol_gold_contract::EXTRA_ROOT_STATE_BYTES
    );

    // Assert that these accounts can be read
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await
        .unwrap();

    assert_eq!(
        auction_root_state.auction_config.cycle_period,
        auction_config.cycle_period
    );
    assert_eq!(
        auction_root_state.auction_config.encore_period,
        auction_config.encore_period
    );
    assert_eq!(auction_root_state.status.current_auction_cycle, 1);
    assert_eq!(auction_root_state.status.current_idle_cycle_streak, 0);
    assert!(auction_cycle_state.bid_history.get_last_element().is_none());
    assert_eq!(auction_root_state.unclaimed_rewards, 0);

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(1, auction_pool.pool.len());
    assert_eq!(auction_pool.pool[0], [123_u8; 32]);

    // Invalid use case
    // Create auction with an id already taken by the same user
    let reinitialize_auction_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(
        reinitialize_auction_error,
        AuctionContractError::AuctionIdNotUnique
    );

    // Invalid use case
    // Create auction with an id already taken by another user
    let other_user = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let initialize_auction_with_same_id_error = initialize_new_auction(
        &mut testbench,
        &other_user.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(
        initialize_auction_with_same_id_error,
        AuctionContractError::AuctionIdNotUnique
    );
}
