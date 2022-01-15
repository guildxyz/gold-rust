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

const CLOSE_AUCTION_CYCLE_COST_EXISTING_MARKER: u64 = 16_557_840;

#[tokio::test]
async fn test_process_claim_funds() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(2),
    };

    let payer = testbench.clone_payer();

    let user_1 = TestUser::new(&mut testbench).await;
    let auction_cycle_payer = TestUser::new(&mut testbench).await.keypair;

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    // Invalid use case
    // Trying to claim from an auction with insufficient treasury
    let claim_amount = 1_000_000;
    let not_enough_funds_to_claim_error = claim_funds_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        claim_amount,
    )
    .await
    .err()
    .unwrap();

    assert_eq!(
        not_enough_funds_to_claim_error,
        AuctionContractError::InvalidClaimAmount
    );

    // Test single bid
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Invalid use case
    // Trying to claim funds from the current auction cycle
    let current_bid_claim_error = claim_funds_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        claim_amount,
    )
    .await
    .err()
    .unwrap();

    assert_eq!(
        current_bid_claim_error,
        AuctionContractError::InvalidClaimAmount
    );

    // Make the auction cycle reach end time
    warp_to_cycle_end(&mut testbench, auction_id).await;

    // Close auction cycle so that we can claim funds
    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLOSE_AUCTION_CYCLE_COST_EXISTING_MARKER + TRANSACTION_FEE,
    );

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert_eq!(auction_root_state.available_funds, bid_amount);
    assert_eq!(auction_root_state.all_time_treasury, bid_amount);

    // This should be successful because the auction cycle of the bid has ended
    let owner_balance_change = claim_funds_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        claim_amount,
    )
    .await
    .unwrap();

    assert_eq!(
        claim_amount / 20 * 19 - TRANSACTION_FEE,
        owner_balance_change as u64
    );

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert_eq!(
        auction_root_state.available_funds,
        bid_amount - claim_amount
    );
    assert_eq!(auction_root_state.all_time_treasury, bid_amount);

    // Close last (second) auction cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Claim funds from the ended auction
    let owner_balance_change = claim_funds_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        claim_amount,
    )
    .await
    .unwrap();

    assert_eq!(
        claim_amount / 20 * 19 - TRANSACTION_FEE,
        owner_balance_change as u64
    );

    // Freeze auction
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert_eq!(
        auction_root_state.available_funds,
        bid_amount - 2 * claim_amount
    );
    assert_eq!(auction_root_state.all_time_treasury, bid_amount);
    assert!(auction_root_state.status.is_frozen);

    // Claim funds from a frozen auction
    let contract_balance_before = get_account_lamports(&mut testbench, &payer.pubkey()).await;
    let owner_balance_change = claim_funds_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        claim_amount,
    )
    .await
    .unwrap();
    let contract_balance_after = get_account_lamports(&mut testbench, &payer.pubkey()).await;

    assert_eq!(
        claim_amount / 20 * 19 - TRANSACTION_FEE,
        owner_balance_change as u64
    );

    assert_eq!(
        claim_amount - (claim_amount / 20 * 19),
        contract_balance_after - contract_balance_before
    );
}
