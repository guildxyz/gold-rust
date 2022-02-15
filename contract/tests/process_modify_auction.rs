#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;

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
        auction_root_state.description.description.contents().to_owned(),
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
            modify_data.new_socials.as_ref().unwrap().contents()[i].contents().to_owned()
        );
    }

    // modify socials
    let modify_data = ModifyAuctionData {
        new_description: None,
        new_socials: None,
        new_encore_period: Some(20000),
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
