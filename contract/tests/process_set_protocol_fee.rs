#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

// This file includes the following tests:
//
// Valid use cases:
//   - Setting the protocol fee with admin signature in valid interval (0% < x <= 5%)
//   - Claiming funds without SetProtocolFee instruction called beforehand
//   - Claiming funds after SetProtocolFee instruction was called
//
// Invalid use cases:
//   - Setting the protocol fee without admin signature
//   - Setting the protocol fee above 5%

#[tokio::test]
async fn test_process_set_protocol_fee() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
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

    let (protocol_fee_state_pubkey, _) =
        Pubkey::find_program_address(&protocol_fee_state_seeds(), &CONTRACT_ID);
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &CONTRACT_ID);

    // Set up some claimable funds to test on
    let bid_amount = 50_000_000;
    place_bid_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        bid_amount,
    )
    .await
    .unwrap()
    .unwrap();

    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

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

    // Test claiming funds without setting protocol fee beforehand
    let claim_amount = 10_000_000;
    claim_and_assert_split(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        claim_amount,
        &contract_bank_pubkey,
        &protocol_fee_state_pubkey,
        50,
    )
    .await;

    // Invalid use case
    // Setting protocol fee without admin signature
    let new_fee = 52;
    let set_fee_without_admin_signature =
        set_protocol_fee_transaction(&mut testbench, &auction_owner.keypair, new_fee)
            .await
            .unwrap()
            .err()
            .unwrap();

    assert_eq!(
        set_fee_without_admin_signature,
        AuctionContractError::ContractAdminMismatch
    );

    // Invalid use case
    // Setting protocol fee to higher than 5%
    let new_fee = 52;
    let protocol_fee_too_damn_high_error =
        set_protocol_fee_transaction(&mut testbench, &payer, new_fee)
            .await
            .unwrap()
            .err()
            .unwrap();

    assert_eq!(
        protocol_fee_too_damn_high_error,
        AuctionContractError::InvalidProtocolFee
    );

    // Creating protocol fee account by setting it to the default value
    let new_fee = 50;
    set_protocol_fee_transaction(&mut testbench, &payer, new_fee)
        .await
        .unwrap()
        .unwrap();

    let claim_amount = 10_000_000;
    claim_and_assert_split(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        claim_amount,
        &contract_bank_pubkey,
        &protocol_fee_state_pubkey,
        new_fee,
    )
    .await;

    // Setting protocol fee to another value
    let new_fee = 10;
    set_protocol_fee_transaction(&mut testbench, &payer, new_fee)
        .await
        .unwrap()
        .unwrap();

    let claim_amount = 10_000_000;
    claim_and_assert_split(
        &mut testbench,
        auction_id,
        &auction_owner.keypair.pubkey(),
        claim_amount,
        &contract_bank_pubkey,
        &protocol_fee_state_pubkey,
        new_fee,
    )
    .await;
}
