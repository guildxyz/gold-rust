#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;

// This file includes the following tests:
//
// Valid use cases:
//   - Modifying auction description
//   - Modifying auction socials
//   - Modifying auction encore period within valid bounds
//
// Invalid use cases:
//   - Modifying auction without owner signature
//   - Modifying auction encore period to invalid value

#[tokio::test]
async fn test_process_modify_auction() {
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

    // modify auction description
    let modify_data = ModifyAuctionData {
        new_description: Some(
            "This is quite the description, definitely changed from the original!"
                .try_into()
                .unwrap(),
        ),
        new_socials: None,
        new_encore_period: None,
    };

    // Invalid use case
    // Try to modify auction without admin signature
    let payer = testbench.clone_payer();
    let no_owner_signature_error =
        modify_auction_transaction(&mut testbench, auction_id, &payer, modify_data.clone())
            .await
            .unwrap()
            .err()
            .unwrap();

    assert_eq!(
        no_owner_signature_error,
        AuctionContractError::AuctionOwnerMismatch
    );

    // Now with correct signature
    let balance_change = modify_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        modify_data.clone(),
    )
    .await
    .unwrap()
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(
        auction_root_state
            .description
            .description
            .contents()
            .to_owned(),
        modify_data.new_description.unwrap().contents().to_owned()
    );

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);

    // modify socials
    let modify_data = ModifyAuctionData {
        new_description: None,
        new_socials: Some(
            vec![
                "an-original-socials-link.ayo".try_into().unwrap(),
                "and-another-one.dj".try_into().unwrap(),
            ]
            .try_into()
            .unwrap(),
        ),
        new_encore_period: None,
    };

    modify_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        modify_data.clone(),
    )
    .await
    .unwrap()
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    for (i, socials_string) in auction_root_state
        .description
        .socials
        .contents()
        .iter()
        .enumerate()
    {
        assert_eq!(
            socials_string.contents().to_owned(),
            modify_data.new_socials.as_ref().unwrap().contents()[i]
                .contents()
                .to_owned()
        );
    }

    // modify encore period

    // Invalid use case
    // Trying to modify encore period over cycle_period/2
    let modify_data = ModifyAuctionData {
        new_description: None,
        new_socials: None,
        new_encore_period: Some(20000),
    };

    let invalid_new_encore_period_error = modify_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        modify_data.clone(),
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        invalid_new_encore_period_error,
        AuctionContractError::InvalidEncorePeriod
    );

    // now with valid encore period
    let modify_data = ModifyAuctionData {
        new_description: None,
        new_socials: None,
        new_encore_period: Some(0),
    };

    modify_auction_transaction(
        &mut testbench,
        auction_id,
        &auction_owner.keypair,
        modify_data.clone(),
    )
    .await
    .unwrap()
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(
        auction_root_state.auction_config.encore_period,
        modify_data.new_encore_period.unwrap()
    );
}
