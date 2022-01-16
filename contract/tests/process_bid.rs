#![cfg(feature = "test-bpf")]

mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::{tokio, Testbench};
use solana_program::pubkey::Pubkey;
use solana_sdk::signer::Signer;

async fn assert_auction_state(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    expected_top_bidder: &Pubkey,
    bid_amount: u64,
) {
    let (_auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await;
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &CONTRACT_ID);

    // Assert top bidder
    if let Some(top_bid) = &get_top_bid(testbench, &auction_cycle_state_pubkey).await {
        assert_eq!(&top_bid.bidder_pubkey, expected_top_bidder);
        assert_eq!(top_bid.bid_amount, bid_amount);
    }

    // Assert fund holding account balance
    let min_balance = testbench.rent.minimum_balance(0);
    assert_eq!(
        min_balance + bid_amount,
        get_account_lamports(testbench, &auction_bank_pubkey).await
    );
}

#[tokio::test]
async fn test_process_bid() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_config = AuctionConfig {
        cycle_period: 100,
        encore_period: 30,
        minimum_bid_amount: 10_000,
        number_of_cycles: Some(10),
    };
    let auction_id = [2; 32];

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    // check state account
    let (auction_root_state_pubkey, _auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await;
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &CONTRACT_ID);

    let initial_funds = get_account_lamports(&mut testbench, &auction_bank_pubkey).await;
    assert!(initial_funds > 0);

    let user_1 = TestUser::new(&mut testbench).await;
    let user_2 = TestUser::new(&mut testbench).await;
    let initial_balance = testbench
        .get_account_lamports(&user_1.keypair.pubkey())
        .await;

    // Invalid use case
    // Test bid lower than minimum_bid
    let lower_than_minimum_bid_error =
        place_bid_transaction(&mut testbench, auction_id, &user_2.keypair, 1000)
            .await
            .err()
            .unwrap();

    assert_eq!(
        lower_than_minimum_bid_error,
        AuctionContractError::InvalidBidAmount
    );

    // Test first bid
    let bid_amount = 1_000_000;
    let balance_change =
        place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
            .await
            .unwrap();

    assert_auction_state(
        &mut testbench,
        auction_id,
        &user_1.keypair.pubkey(),
        bid_amount,
    )
    .await;

    // Assert balances
    assert_eq!(-balance_change as u64, bid_amount + TRANSACTION_FEE);

    // Check if treasury is updated
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert_eq!(auction_root_state.all_time_treasury, bid_amount);

    // Test higher than current bid
    let bid_amount_higher = 2_000_000;
    let balance_change = place_bid_transaction(
        &mut testbench,
        auction_id,
        &user_2.keypair,
        bid_amount_higher,
    )
    .await
    .unwrap();

    assert_auction_state(
        &mut testbench,
        auction_id,
        &user_2.keypair.pubkey(),
        bid_amount_higher,
    )
    .await;

    // Assert balances
    assert_eq!(
        initial_balance - TRANSACTION_FEE,
        get_account_lamports(&mut testbench, &user_1.keypair.pubkey()).await
    );

    assert_eq!(-balance_change as u64, bid_amount_higher + TRANSACTION_FEE);

    // Check if treasury is updated
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert_eq!(auction_root_state.all_time_treasury, bid_amount_higher);

    // Invalid use case
    // Test bid lower than current bid
    let bid_amount_lower = 100_000;
    let lower_bid_error = place_bid_transaction(
        &mut testbench,
        auction_id,
        &user_2.keypair,
        bid_amount_lower,
    )
    .await
    .err()
    .unwrap();

    assert_eq!(lower_bid_error, AuctionContractError::InvalidBidAmount);
    assert_auction_state(
        &mut testbench,
        auction_id,
        &user_2.keypair.pubkey(),
        bid_amount_higher,
    )
    .await;

    // Invalid use case
    // Test bid into expired auction
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let bid_amount = 10_000_000;
    let bid_to_expired_auction_error =
        place_bid_transaction(&mut testbench, auction_id, &user_2.keypair, bid_amount)
            .await
            .err()
            .unwrap();

    assert_eq!(
        bid_to_expired_auction_error,
        AuctionContractError::AuctionCycleEnded
    );

    assert_auction_state(
        &mut testbench,
        auction_id,
        &user_2.keypair.pubkey(),
        bid_amount_higher,
    )
    .await;

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &CONTRACT_ID);
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(1, auction_pool.pool.len());
    assert_eq!(
        auction_pool.pool.get(&auction_id).unwrap(),
        &auction_root_state_pubkey
    );
}

#[tokio::test]
async fn bid_to_frozen_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let user_2 = TestUser::new(&mut testbench).await;

    let auction_id = [2; 32];
    let auction_config = AuctionConfig {
        cycle_period: 100,
        encore_period: 30,
        minimum_bid_amount: 10_000,
        number_of_cycles: Some(10),
    };

    // Test bid into frozen auction
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(!auction_root_state.status.is_frozen);
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.status.is_frozen);

    let bid_amount = 1_000_000;
    let bid_to_frozen_auction_error =
        place_bid_transaction(&mut testbench, auction_id, &user_2.keypair, bid_amount)
            .await
            .err()
            .unwrap();

    assert_eq!(
        bid_to_frozen_auction_error,
        AuctionContractError::AuctionFrozen
    );

    // Test bid on frozen AND expired auction
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let bid_to_frozen_and_expired_auction_error =
        place_bid_transaction(&mut testbench, auction_id, &user_2.keypair, bid_amount)
            .await
            .err()
            .unwrap();

    assert_eq!(
        bid_to_frozen_and_expired_auction_error,
        AuctionContractError::AuctionFrozen
    );
}

#[tokio::test]
async fn test_encore_bid() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [2; 32];
    let auction_config = AuctionConfig {
        cycle_period: 1000,
        encore_period: 200,
        minimum_bid_amount: 10_000,
        number_of_cycles: Some(10),
    };

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let user = TestUser::new(&mut testbench).await;

    // check state account
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;

    // Place bid to trigger encore
    testbench.warp_n_seconds(900).await;

    // Assert that we should trigger encore with the bid
    assert!(
        testbench.block_time().await
            > auction_cycle_state.end_time - auction_root_state.auction_config.encore_period
    );
    assert!(testbench.block_time().await < auction_cycle_state.end_time);
    let end_time_before = auction_cycle_state.end_time;

    let bid_amount = 2_000_000;
    let block_time_before = testbench.block_time().await;
    place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount)
        .await
        .unwrap();

    // Fetch cycle state again (updated by the transaction)
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;
    let end_time_after = auction_cycle_state.end_time;

    assert!(end_time_after > end_time_before);
    assert!(end_time_after < end_time_before + auction_root_state.auction_config.encore_period);

    // This test is theoretically true, but the BanksClient works in mysterious ways
    // May need to comment this out later
    assert_eq!(
        end_time_after,
        block_time_before + auction_root_state.auction_config.encore_period
    );
}
