#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;

use agsol_gold_contract::instruction::factory::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_gold_contract::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL;

use agsol_testbench::tokio;
use agsol_testbench::Testbench;

#[tokio::test]
async fn test_delete_small_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
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
    .unwrap()
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(&mut testbench, auction_id, &auction_owner, &payer, 3, 100).await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_auction_cycle, 4);

    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(
            &auction_root_state_pubkey,
            &auction_root_state
                .status
                .current_auction_cycle
                .to_le_bytes(),
        ),
        &CONTRACT_ID,
    );

    let delete_auction_args = DeleteAuctionArgs {
        auction_owner_pubkey: auction_owner.keypair.pubkey(),
        top_bidder_pubkey: get_top_bidder_pubkey(&mut testbench, &auction_cycle_state_pubkey)
            .await
            .unwrap(),
        auction_id,
        current_auction_cycle: get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap(),
        num_of_cycles_to_delete: RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    };
    let delete_auction_ix = delete_auction(&delete_auction_args);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 1);
    testbench
        .process_transaction(&[delete_auction_ix], &auction_owner.keypair, None)
        .await
        .unwrap()
        .unwrap();

    // Test if auction was removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert!(auction_pool.pool.is_empty());

    // Test if state accounts are deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 4).await);
}

#[tokio::test]
async fn test_delete_just_long_enough_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
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
    .unwrap()
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
        1000,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert!(auction_root_state.status.is_finished);

    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(
            &auction_root_state_pubkey,
            &auction_root_state
                .status
                .current_auction_cycle
                .to_le_bytes(),
        ),
        &CONTRACT_ID,
    );

    let delete_auction_args = DeleteAuctionArgs {
        auction_owner_pubkey: auction_owner.keypair.pubkey(),
        top_bidder_pubkey: get_top_bidder_pubkey(&mut testbench, &auction_cycle_state_pubkey)
            .await
            .unwrap(),
        auction_id,
        current_auction_cycle: get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap(),
        num_of_cycles_to_delete: RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    };
    let delete_auction_ix = delete_auction(&delete_auction_args);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 1);
    testbench
        .process_transaction(&[delete_auction_ix.clone()], &auction_owner.keypair, None)
        .await
        .unwrap()
        .unwrap();

    // Test that auction is removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert!(auction_pool.pool.is_empty());

    // Test that state accounts are also deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(
        are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 30).await
    );
}

#[tokio::test]
async fn test_delete_long_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
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
    .unwrap()
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_n_cycles(&mut testbench, auction_id, &auction_owner, &payer, 31, 1000).await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await.unwrap();

    assert!(auction_root_state.status.is_finished);

    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(
            &auction_root_state_pubkey,
            &auction_root_state
                .status
                .current_auction_cycle
                .to_le_bytes(),
        ),
        &CONTRACT_ID,
    );

    let mut delete_auction_args = DeleteAuctionArgs {
        auction_owner_pubkey: auction_owner.keypair.pubkey(),
        top_bidder_pubkey: get_top_bidder_pubkey(&mut testbench, &auction_cycle_state_pubkey)
            .await
            .unwrap(),
        auction_id,
        current_auction_cycle: get_current_cycle_number(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap(),
        num_of_cycles_to_delete: RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    };
    let delete_auction_ix = delete_auction(&delete_auction_args);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await.unwrap();
    assert_eq!(auction_pool.pool.len(), 1);
    testbench
        .process_transaction(&[delete_auction_ix], &auction_owner.keypair, None)
        .await.unwrap()
        .unwrap();

    // Test that auction is not yet removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await.unwrap();
    assert_eq!(auction_pool.pool.len(), 1); // should still be present

    // Test that state accounts are not deleted
    assert!(is_existing_account(&mut testbench, &auction_root_state_pubkey).await.unwrap());
    assert!(is_existing_account(&mut testbench, &auction_bank_pubkey).await.unwrap());
    assert!(
        are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 2, 31).await
    );
    assert!(does_nth_cycle_state_exist(&mut testbench, &auction_root_state_pubkey, 1).await);

    delete_auction_args.current_auction_cycle =
        get_current_cycle_number(&mut testbench, &auction_root_state_pubkey).await.unwrap();
    let delete_auction_ix = delete_auction(&delete_auction_args);
    testbench
        .process_transaction(&[delete_auction_ix], &auction_owner.keypair, None)
        .await
        .unwrap()
        .unwrap();

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await.unwrap();
    assert!(auction_pool.pool.is_empty()); // should be deleted now

    // Test that state accounts are now deleted
    assert!(!is_existing_account(&mut testbench, &auction_root_state_pubkey).await.unwrap());
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey).await.unwrap());
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 1).await);
}

async fn does_nth_cycle_state_exist(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
    n: u64,
) -> bool {
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(auction_root_state_pubkey, &n.to_le_bytes()),
        &CONTRACT_ID,
    );
    is_existing_account(testbench, &auction_cycle_state_pubkey)
        .await
        .unwrap()
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
    _current_slot_estimate: u64,
) {
    for _ in 0..n {
        place_bid_transaction(testbench, auction_id, payer, 50_000_000)
            .await
            .unwrap()
            .unwrap();

        warp_to_cycle_end(testbench, auction_id).await.unwrap();

        close_cycle_transaction(
            testbench,
            payer,
            auction_id,
            &auction_owner.keypair.pubkey(),
            TokenType::Nft,
        )
        .await
        .unwrap()
        .unwrap();
    }
}
