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

#[tokio::test]
async fn test_process_freeze() {
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

    let payer = testbench.clone_payer();

    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &CONTRACT_ID);
    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(!auction_root_state.status.is_frozen);

    let bidder = TestUser::new(&mut testbench).await.unwrap().unwrap();

    // Bid to auction once
    let first_bid = 50_000_000;
    let balance_change =
        place_bid_transaction(&mut testbench, auction_id, &bidder.keypair, first_bid)
            .await
            .unwrap()
            .unwrap();

    assert_eq!(-balance_change as u64, first_bid + TRANSACTION_FEE);

    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    // close first cycle
    close_cycle_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.available_funds, first_bid);
    assert_eq!(auction_root_state.all_time_treasury, first_bid);

    // bid to second cycle
    let second_bid = 50_000_000;
    let balance_change =
        place_bid_transaction(&mut testbench, auction_id, &bidder.keypair, second_bid)
            .await
            .unwrap()
            .unwrap();

    assert_eq!(-balance_change as u64, second_bid + TRANSACTION_FEE);

    let bidder_balance = testbench
        .get_account_lamports(&bidder.keypair.pubkey())
        .await
        .unwrap();
    let owner_balance = testbench
        .get_account_lamports(&auction_owner.keypair.pubkey())
        .await
        .unwrap();
    let contract_balance = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();
    let withdrawn_amount = testbench
        .get_account_lamports(&auction_bank_pubkey)
        .await
        .unwrap()
        - second_bid;

    // Freezing auction
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap()
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert!(auction_root_state.status.is_frozen);
    assert_eq!(auction_root_state.all_time_treasury, first_bid);
    assert_eq!(auction_root_state.available_funds, 0);

    let bidder_balance_after = testbench
        .get_account_lamports(&bidder.keypair.pubkey())
        .await
        .unwrap();
    let owner_balance_after = testbench
        .get_account_lamports(&auction_owner.keypair.pubkey())
        .await
        .unwrap();
    let contract_balance_after = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();

    let five_percent = withdrawn_amount / 20;
    assert_eq!(bidder_balance_after, bidder_balance + second_bid); // bidder refunded
    assert_eq!(
        owner_balance_after,
        owner_balance - TRANSACTION_FEE + 19 * five_percent
    );
    assert_eq!(contract_balance_after, contract_balance + five_percent);

    // Invalid use case
    // Freezing already frozen auction
    let auction_already_frozen_error =
        freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
            .await
            .unwrap()
            .err()
            .unwrap();
    assert_eq!(
        auction_already_frozen_error,
        AuctionContractError::AuctionFrozen
    );

    // Invalid use case
    // Freeze / thaw without correct signature
    let freeze_incorrect_signature_error =
        freeze_auction_transaction(&mut testbench, auction_id, &payer)
            .await
            .unwrap()
            .err()
            .unwrap();
    assert_eq!(
        freeze_incorrect_signature_error,
        AuctionContractError::AuctionOwnerMismatch
    );
}
