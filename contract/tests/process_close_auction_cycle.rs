#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ALLOWED_CONSECUTIVE_IDLE_CYCLES;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::{tokio, TestbenchError};
use agsol_token_metadata::instruction::CreateMetadataAccountArgs;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

const CLOSE_CYCLE_COST: u64 = 3_758_400;
const CLOSE_LAST_CYCLE_COST: u64 = 0;

// This file includes the following tests:
//
// Valid use cases:
//   - Closing cycles on nft auctions after they ended
//   - Closing cycles on repeating and non-repeating nft auctions
//   - Closing cycle on auction with no bid placed
//   - Closing cycles until auction is moved to the secondary pool
//   - Bidding on idle auction which is consequently moved to the primary pool
//
// Invalid use cases:
//   - Bidding on finished auction
//   - Closing cycle on finished auction

#[tokio::test]
async fn test_process_close_auction_cycle() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    let (auction_cycle_state_pubkey, auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap();

    // Test no bids were taken
    // Close first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);

    // Check if idle cycle streak has been incremented
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_idle_cycle_streak, 1);

    let (same_auction_cycle_state_pubkey, same_auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap();

    // Check if auction timings were correctly updated
    assert_eq!(auction_cycle_state_pubkey, same_auction_cycle_state_pubkey);
    assert_eq!(
        auction_cycle_state.end_time + auction_config.cycle_period,
        same_auction_cycle_state.end_time
    );
    let current_time = testbench.block_time().await.unwrap();
    assert!(current_time < same_auction_cycle_state.end_time);

    // Check no nft was minted
    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    assert_eq!(
        testbench
            .get_token_account(&child_edition.holding)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );
    assert_eq!(
        testbench
            .get_mint_account(&child_edition.mint)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );

    // Check if other data are unchanged
    assert_eq!(
        auction_root_state.auction_config.cycle_period,
        auction_config.cycle_period
    );
    assert_eq!(
        auction_root_state.auction_config.encore_period,
        auction_config.encore_period
    );
    assert_eq!(auction_root_state.available_funds, 0);
    assert_eq!(auction_root_state.all_time_treasury, 0);
    assert!(auction_cycle_state.bid_history.get_last_element().is_none());

    // Test some bids were taken
    // Place bid
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close first cycle after bid
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, CLOSE_CYCLE_COST + TRANSACTION_FEE,);

    // Check if idle cycle streak has been reset
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_idle_cycle_streak, 0);

    // Check new cycle end time
    let new_cycle_min_end_time =
        auction_cycle_state.end_time + auction_root_state.auction_config.cycle_period;
    let (_auction_cycle_state_pubkey, auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap();
    assert!(auction_cycle_state.end_time >= new_cycle_min_end_time);

    // Check if fund information is correctly updated
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.available_funds, bid_amount);
    assert_eq!(auction_root_state.all_time_treasury, bid_amount);

    // Check no nft was minted
    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    assert_eq!(
        testbench
            .get_token_account(&child_edition.holding)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );
    assert_eq!(
        testbench
            .get_mint_account(&child_edition.mint)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );
}

#[tokio::test]
async fn test_close_cycle_on_finished_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1),
    };

    let user = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Place bid
    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close first (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLOSE_LAST_CYCLE_COST + TRANSACTION_FEE,
    );

    let (auction_root_state_pubkey, _auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await.unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_finished);

    // Invalid use case
    // Bidding on ended auction
    let bid_amount_higher = 80_000_000;
    let bid_on_ended_auction_error =
        place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount_higher)
            .await
            .unwrap()
            .err()
            .unwrap();

    assert_eq!(
        bid_on_ended_auction_error,
        AuctionContractError::AuctionEnded
    );

    // Invalid use case
    // Closing cycle of ended auction
    let close_cycle_on_ended_auction_error = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(
        close_cycle_on_ended_auction_error,
        AuctionContractError::AuctionEnded
    );
}

#[tokio::test]
async fn test_close_cycle_child_metadata_change_not_repeating() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(3),
    };

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check initial master metadata
    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    assert_metadata_uri(&mut testbench, &master_edition, "1.json").await;

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &CONTRACT_ID);
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool[0], auction_id);
    assert!(secondary_pool.pool.is_empty());

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();

    // Place bid on first cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check master metadata is incremented
    assert_metadata_uri(&mut testbench, &master_edition, "2.json").await;

    // Place bid on second cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check master metadata is incremented
    assert_metadata_uri(&mut testbench, &master_edition, "3.json").await;

    // Place bid on last cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the third (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check master metadata is reset after closing last cycle
    assert_metadata_uri(&mut testbench, &master_edition, "0.json").await;

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(secondary_pool.pool[0], auction_id);
    assert!(auction_pool.pool.is_empty());
}

#[tokio::test]
async fn test_child_close_cycle_metadata_change_repeating() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(3),
    };

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let create_token_args = CreateTokenArgs::Nft {
        metadata_args: CreateMetadataAccountArgs {
            data: agsol_token_metadata::state::Data {
                name: "random auction".to_owned(),
                symbol: "RAND".to_owned(),
                uri: "uri".to_owned(),
                seller_fee_basis_points: 10,
                creators: None,
            },
            is_mutable: true,
        },
        is_repeating: true,
    };

    initialize_new_auction_custom(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        create_token_args,
    )
    .await
    .unwrap()
    .unwrap();

    // Check initial master metadata
    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    assert_metadata_uri(&mut testbench, &master_edition, "0.json").await;

    // Check initial cycle number
    let current_cycle = get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(current_cycle, 1);

    // bid to first cycle
    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check current cycle
    let current_cycle = get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(current_cycle, 2);

    // Check master metadata uri is unchanged
    assert_metadata_uri(&mut testbench, &master_edition, "0.json").await;

    // bid to second cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check current cycle
    let current_cycle = get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(current_cycle, 3);

    // Check master metadata uri is unchanged
    assert_metadata_uri(&mut testbench, &master_edition, "0.json").await;

    // bid to the last cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the third (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check current cycle
    let current_cycle = get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(current_cycle, 3);

    // Check master metadata uri is unchanged
    assert_metadata_uri(&mut testbench, &master_edition, "0.json").await;
}

#[tokio::test]
async fn test_process_close_idle_rapid_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    let mut auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    // Close idle cycles until moved to secondary pool
    for cycle_number in 1..(ALLOWED_CONSECUTIVE_IDLE_CYCLES + 2) {
        assert!(!auction_root_state.status.is_filtered);
        warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

        let balance_change = close_cycle_transaction(
            &mut testbench,
            &auction_cycle_payer,
            auction_id,
            &auction_owner.keypair.pubkey(),
            TokenType::Nft,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(-balance_change as u64, TRANSACTION_FEE);

        // Check if idle cycle streak has been incremented
        auction_root_state = testbench
            .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
            .await
            .unwrap();

        assert_eq!(
            auction_root_state.status.current_idle_cycle_streak,
            cycle_number
        );
    }

    let (primary_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &CONTRACT_ID);

    let primary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&primary_pool_pubkey)
        .await
        .unwrap();
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();

    assert_eq!(primary_pool.pool.len(), 0);
    assert_eq!(secondary_pool.pool.len(), 1);

    // Warp well over expiration period
    testbench
        .warp_n_seconds(auction_config.cycle_period * 2)
        .await
        .unwrap();

    let (_, auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await.unwrap();

    // Bid on idle auction
    let time_before = testbench.block_time().await.unwrap();

    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &auction_cycle_payer, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Check cycle end time
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await
        .unwrap();
    let end_time_after = auction_cycle_state.end_time;

    assert!(end_time_after > time_before);
    assert!(end_time_after <= time_before + auction_root_state.auction_config.cycle_period);

    // Check that the auction is reactivated
    let primary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&primary_pool_pubkey)
        .await
        .unwrap();
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();

    assert_eq!(primary_pool.pool.len(), 1);
    assert_eq!(secondary_pool.pool.len(), 0);
}
