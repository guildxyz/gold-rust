#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::instruction::factory::TokenType;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;

// This file includes the following tests:
//
// Valid use cases:
//   - Filtering and unfiltering an auction
//
// Invalid use cases:
//   - Filtering without contract admin signature

#[tokio::test]
async fn test_process_filter_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [2; 32];
    let auction_config = AuctionConfig {
        cycle_period: 100,
        encore_period: 30,
        minimum_bid_amount: 50_000_000,
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
    .unwrap()
    .unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &CONTRACT_ID);

    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(!auction_root_state.status.is_filtered);

    // Invalid use case
    // Filtering without contract admin signature
    let random_user = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let filter_without_admin_signature_error =
        filter_auction_transaction(&mut testbench, auction_id, true, &random_user.keypair)
            .await
            .unwrap()
            .err()
            .unwrap();
    assert_eq!(
        filter_without_admin_signature_error,
        AuctionContractError::ContractAdminMismatch
    );

    // filter auction
    let payer = testbench.clone_payer();
    filter_auction_transaction(&mut testbench, auction_id, true, &payer)
        .await
        .unwrap()
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_filtered);

    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(secondary_pool.pool[0], auction_id);
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert!(auction_pool.pool.is_empty());

    // unfilter
    filter_auction_transaction(&mut testbench, auction_id, false, &payer)
        .await
        .unwrap()
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(!auction_root_state.status.is_filtered);

    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();

    assert!(secondary_pool.pool.is_empty());
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool[0], auction_id);
}
