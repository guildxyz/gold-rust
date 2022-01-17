#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;
use solana_sdk::signer::Signer;

const TRANSACTION_FEE: u64 = 5000;

#[tokio::test]
async fn test_process_freeze_thaw() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [2; 32];
    let auction_config = AuctionConfig {
        cycle_period: 100,
        encore_period: 30,
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

    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(!auction_root_state.status.is_frozen);

    // Bid to auction once
    let user = TestUser::new(&mut testbench).await;
    let initial_balance = 150_000_000;
    assert_eq!(
        initial_balance,
        testbench.get_account_lamports(&user.keypair.pubkey()).await
    );

    let bid_amount = 10_000_000;
    let balance_change =
        place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount)
            .await
            .unwrap();

    assert_eq!(-balance_change as u64, bid_amount + TRANSACTION_FEE);

    // Freezing auction
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.status.is_frozen);
    assert_eq!(auction_root_state.all_time_treasury, 0);
    assert_eq!(
        initial_balance - TRANSACTION_FEE,
        testbench.get_account_lamports(&user.keypair.pubkey()).await
    );

    // Invalid use case
    // Freezing already frozen auction
    let auction_already_frozen_error =
        freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
            .await
            .err()
            .unwrap();
    assert_eq!(
        auction_already_frozen_error,
        AuctionContractError::AuctionFrozen
    );

    // Thaw auction
    let payer = testbench.clone_payer();
    thaw_auction_transaction(&mut testbench, auction_id, &payer)
        .await
        .unwrap();

    // check if auction was thawed
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(!auction_root_state.status.is_frozen);

    // Valid use case but does not do anything
    // Thaw not frozen auction
    thaw_auction_transaction(&mut testbench, auction_id, &payer)
        .await
        .unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(!auction_root_state.status.is_frozen);

    // Invalid use case
    // Freeze / thaw without correct signature
    let freeze_incorrect_signature_error =
        freeze_auction_transaction(&mut testbench, auction_id, &payer)
            .await
            .err()
            .unwrap();
    assert_eq!(
        freeze_incorrect_signature_error,
        AuctionContractError::AuctionOwnerMismatch
    );

    let thaw_incorrect_signature_error =
        thaw_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
            .await
            .err()
            .unwrap();
    assert_eq!(
        thaw_incorrect_signature_error,
        AuctionContractError::ContractAdminMismatch
    );
}