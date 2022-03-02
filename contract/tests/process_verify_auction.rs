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
//   - Verifying an auction with admin signature
//   - Verifying an already verified auction
//
// Invalid use cases:
//   - Verifying without contract admin signature

#[tokio::test]
async fn test_process_verify_auction() {
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

    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(!auction_root_state.status.is_verified);

    // Invalid use case
    // Verifying without admin signature
    let verify_without_admin_signature =
        verify_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
            .await
            .unwrap()
            .err()
            .unwrap();

    assert_eq!(
        verify_without_admin_signature,
        AuctionContractError::ContractAdminMismatch
    );

    // Verifying auction
    let payer = testbench.clone_payer();
    let balance_change = verify_auction_transaction(&mut testbench, auction_id, &payer)
        .await
        .unwrap()
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_verified);

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);

    // Verifying already Verified auction
    // NOTE: has no effect
    verify_auction_transaction(&mut testbench, auction_id, &payer)
        .await
        .unwrap()
        .unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_verified);
}
