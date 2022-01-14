#![cfg(feature = "test-bpf")]
mod test_factory;

use agsol_testbench::Testbench;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_gold_contract::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL;
use agsol_testbench::tokio;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;

#[tokio::test]
async fn test_delete_small_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 300,
        encore_period: 10,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(10),
    };

    let payer = testbench.clone_payer();

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        3,
        auction_config.cycle_period,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;

    assert_eq!(auction_root_state.status.current_auction_cycle, 4);

    // Invalid use case
    // Trying to delete active auction
    let delete_active_auction_error = delete_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        &payer,
    )
    .await
    .err()
    .unwrap();

    assert_eq!(
        delete_active_auction_error,
        AuctionContractError::AuctionIsActive,
    );

    // Freeze auction so it can be deleted
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(auction_pool.pool.len(), 1);

    delete_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        &payer,
    )
    .await
    .unwrap();

    // Test if auction was removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert!(auction_pool.pool.is_empty());

    // Test if state accounts are deleted
    assert!(!is_existing_account(&mut testbench, &auction_root_state_pubkey).await);
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey).await);
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 4).await);
}

// Very similar to the previous test
// Only difference is that instead of freezing the auction it becomes inactive when closing last cycle
#[tokio::test]
async fn test_delete_inactive_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 300,
        encore_period: 10,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(3),
    };

    let payer = testbench.clone_payer();

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        3,
        auction_config.cycle_period,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;

    assert!(!auction_root_state.status.is_active);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(auction_pool.pool.len(), 1);

    delete_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        &payer,
    )
    .await
    .unwrap();

    // Test if auction was removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert!(auction_pool.pool.is_empty());

    // Test if state accounts are deleted
    assert!(!is_existing_account(&mut testbench, &auction_root_state_pubkey).await);
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey).await);
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 3).await);
}

#[tokio::test]
async fn test_delete_just_long_enough_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 300,
        encore_period: 10,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL),
    };

    let payer = testbench.clone_payer();

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
        auction_config.cycle_period,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;

    assert!(!auction_root_state.status.is_active);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(auction_pool.pool.len(), 1);

    delete_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        &payer,
    )
    .await
    .unwrap();

    // Test that auction is removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert!(auction_pool.pool.is_empty());

    // Test that state accounts are also deleted
    assert!(!is_existing_account(&mut testbench, &auction_root_state_pubkey).await);
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey).await);
    assert!(
        are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 30).await
    );
}

#[tokio::test]
async fn test_delete_long_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 300,
        encore_period: 10,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL + 1),
    };

    let payer = testbench.clone_payer();

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&get_auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&get_auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        31,
        auction_config.cycle_period,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;

    assert!(!auction_root_state.status.is_active);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(auction_pool.pool.len(), 1);

    delete_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        &payer,
    )
    .await
    .unwrap();

    // Test that auction is not yet removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(auction_pool.pool.len(), 1); // should still be present

    // Test that state accounts are not deleted
    assert!(is_existing_account(&mut testbench, &auction_root_state_pubkey).await);
    assert!(is_existing_account(&mut testbench, &auction_bank_pubkey).await);
    assert!(
        are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 2, 31).await
    );
    assert!(does_nth_cycle_state_exist(&mut testbench, &auction_root_state_pubkey, 1).await);

    delete_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        &payer,
    )
    .await
    .unwrap();

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert!(auction_pool.pool.is_empty()); // should be deleted now

    // Test that state accounts are now deleted
    assert!(!is_existing_account(&mut testbench, &auction_root_state_pubkey).await);
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey).await);
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 1).await);
}

async fn does_nth_cycle_state_exist(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
    n: u64,
) -> bool {
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &get_auction_cycle_state_seeds(auction_root_state_pubkey, &n.to_le_bytes()),
        &CONTRACT_ID,
    );
    is_existing_account(testbench, &auction_cycle_state_pubkey).await
}

async fn are_given_cycle_states_deleted(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
    from: u64,
    to: u64,
) -> bool {
    for i in from..=to {
        if does_nth_cycle_state_exist(testbench, auction_root_state_pubkey, i).await {
            return false;
        }
    }
    true
}

async fn close_n_cycles(
    testbench: &mut Testbench,
    auction_id: AuctionId,
    auction_owner: &TestUser,
    payer: &Keypair,
    n: u64,
    _cycle_period: i64,
) {
    let bid_amount = 100_000;
    for _ in 0..n {
        place_bid_transaction(testbench, auction_id, payer, bid_amount)
            .await
            .unwrap();

        // NOTE: This would be more robust but slower
        warp_to_cycle_end(testbench, auction_id).await;

        //testbench.warp_n_seconds(cycle_period).await;
        // let pre_cycle_end_time = testbench.block_time().await;

        close_cycle_transaction(
            testbench,
            payer,
            auction_id,
            &auction_owner.keypair.pubkey(),
            TokenType::Nft,
        )
        .await
        .unwrap();

        // These might come handy later as well...
        // let post_cycle_end_time = testbench.block_time().await;
        // dbg!(post_cycle_end_time - pre_cycle_end_time);
    }
}
