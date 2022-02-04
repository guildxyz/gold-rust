#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::processor::increment_uri;
use agsol_gold_contract::state::*;
use agsol_gold_contract::utils::unpuff_metadata;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ALLOWED_CONSECUTIVE_IDLE_CYCLES;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::{tokio, TestbenchError};
use agsol_token_metadata::instruction::CreateMetadataAccountArgs;
use agsol_token_metadata::state::Metadata;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

const CLOSE_AUCTION_CYCLE_LAST_CYCLE: u64 = 12_799_440;
const CLOSE_AUCTION_CYCLE_COST_EXISTING_MARKER: u64 = 16_557_840;

#[tokio::test]
async fn test_process_close_auction_cycle() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

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

    let (auction_cycle_state_pubkey, auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap();

    // Test no bids were taken
    // Close first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);

    // Check if idle cycle streak has been incremented
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_idle_cycle_streak, 1);

    let (same_auction_cycle_state_pubkey, same_auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap();

    // Check if auction timings were correctly updated
    assert_eq!(auction_cycle_state_pubkey, same_auction_cycle_state_pubkey);
    assert_eq!(
        auction_cycle_state.end_time + auction_config.cycle_period,
        same_auction_cycle_state.end_time
    );
    let current_time = testbench.block_time().await.unwrap();
    assert!(current_time < same_auction_cycle_state.end_time);

    // Check no nft was minted
    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    assert_eq!(
        testbench
            .get_token_account(&child_edition.holding)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );
    assert_eq!(
        testbench
            .get_mint_account(&child_edition.mint)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );

    // Check if other data are unchanged
    assert_eq!(
        auction_root_state.auction_config.cycle_period,
        auction_config.cycle_period
    );
    assert_eq!(
        auction_root_state.auction_config.encore_period,
        auction_config.encore_period
    );
    assert_eq!(auction_root_state.available_funds, 0);
    assert_eq!(auction_root_state.all_time_treasury, 0);
    assert!(auction_cycle_state.bid_history.get_last_element().is_none());

    // Test some bids were taken
    // Place bid
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLOSE_AUCTION_CYCLE_COST_EXISTING_MARKER + TRANSACTION_FEE,
    );

    // Check if idle cycle streak has been reset
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_idle_cycle_streak, 0);

    // Check new cycle end time
    let new_cycle_min_end_time =
        auction_cycle_state.end_time + auction_root_state.auction_config.cycle_period;
    let (_auction_cycle_state_pubkey, auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap();

    // Check if asset holding is created and asset is minted
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let user_1_nft_account = testbench
        .get_token_account(&child_edition.holding)
        .await
        .unwrap();
    let child_mint_account = testbench
        .get_mint_account(&child_edition.mint)
        .await
        .unwrap();
    assert_eq!(user_1_nft_account.mint, child_edition.mint);
    assert_eq!(user_1_nft_account.owner, user_1.keypair.pubkey());
    assert_eq!(user_1_nft_account.amount, 1);
    assert_eq!(child_mint_account.supply, 1);

    assert!(auction_cycle_state.bid_history.get_last_element().is_none());
    assert!(auction_cycle_state.end_time >= new_cycle_min_end_time);

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.available_funds, bid_amount);
    assert_eq!(auction_root_state.all_time_treasury, bid_amount);
}

#[tokio::test]
async fn test_ended_close_cycle_on_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1),
    };

    let user = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

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

    // Place bid
    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close first (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLOSE_AUCTION_CYCLE_LAST_CYCLE + TRANSACTION_FEE,
    );

    let (auction_root_state_pubkey, _auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await.unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_finished);

    // Invalid use case
    // Bidding on ended auction
    let bid_amount_higher = 80_000_000;
    let bid_on_ended_auction_error =
        place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount_higher)
            .await
            .unwrap()
            .err()
            .unwrap();

    assert_eq!(
        bid_on_ended_auction_error,
        AuctionContractError::AuctionEnded
    );

    // Invalid use case
    // Closing cycle of ended auction
    let close_cycle_on_ended_auction_error = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(
        close_cycle_on_ended_auction_error,
        AuctionContractError::AuctionEnded
    );

    // Invalid use case
    // Freezing ended auction
    let freeze_finished_auction_error =
        freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
            .await
            .unwrap()
            .err()
            .unwrap();
    assert_eq!(
        freeze_finished_auction_error,
        AuctionContractError::AuctionEnded
    );
}

#[tokio::test]
async fn test_close_cycle_on_frozen_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1),
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

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;
    let (auction_root_state_pubkey, _auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await.unwrap();

    // Freeze auction
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap()
        .unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_frozen);

    // Invalid use case
    // End cycle on frozen auction

    // Warp to slot so that the cycle could be closed if it was not frozen
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    // Trying to close the cycle
    let close_cycle_on_frozen_auction_error = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();
    assert_eq!(
        close_cycle_on_frozen_auction_error,
        AuctionContractError::AuctionFrozen
    );
}

#[tokio::test]
async fn test_close_cycle_child_metadata_change_not_repeating() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(3),
    };

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

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

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();

    // Place bid on first cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    let master_metadata_before = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();
    let child_metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&child_edition.metadata)
        .await
        .unwrap();

    check_metadata_update(
        &master_metadata_before,
        &master_metadata_after,
        &child_metadata,
        false,
    );

    // Place bid on second cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    let master_metadata_before = master_metadata_after.clone();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 2);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();
    let child_metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&child_edition.metadata)
        .await
        .unwrap();

    check_metadata_update(
        &master_metadata_before,
        &master_metadata_after,
        &child_metadata,
        false,
    );

    // Place bid on last cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the third (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    let master_metadata_before = master_metadata_after.clone();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 3);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();
    let child_metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&child_edition.metadata)
        .await
        .unwrap();

    check_metadata_update(
        &master_metadata_before,
        &master_metadata_after,
        &child_metadata,
        true,
    );
}

#[tokio::test]
async fn test_child_close_cycle_metadata_change_repeating() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(3),
    };

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let create_token_args = CreateTokenArgs::Nft {
        metadata_args: CreateMetadataAccountArgs {
            data: agsol_token_metadata::state::Data {
                name: "random auction".to_owned(),
                symbol: "RAND".to_owned(),
                uri: "uri".to_owned(),
                seller_fee_basis_points: 10,
                creators: None,
            },
            is_mutable: true,
        },
        is_repeating: true,
    };

    initialize_new_auction_custom(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        create_token_args,
    )
    .await
    .unwrap()
    .unwrap();

    // bid to first cycle
    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    let master_metadata_before = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 1);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();

    assert_eq!(
        master_metadata_after.data.name,
        master_metadata_before.data.name
    );
    assert_eq!(
        master_metadata_after.data.uri,
        master_metadata_before.data.uri
    );

    // bid to second cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    let master_metadata_before = master_metadata_after.clone();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 2);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();

    assert_eq!(
        master_metadata_after.data.name,
        master_metadata_before.data.name
    );
    assert_eq!(
        master_metadata_after.data.uri,
        master_metadata_before.data.uri
    );

    // bid to the last cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close the third (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 3);
    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await
        .unwrap();

    assert_eq!(
        master_metadata_after.data.name,
        master_metadata_before.data.name
    );
    assert_eq!(
        master_metadata_after.data.uri,
        master_metadata_before.data.uri
    );
}

fn check_metadata_update(
    master_metadata_before: &Metadata,
    master_metadata_after: &Metadata,
    child_metadata: &Metadata,
    is_last_cycle: bool,
) {
    let mut master_metadata_before = master_metadata_before.data.clone();
    let mut master_metadata_after = master_metadata_after.data.clone();
    let child_metadata = child_metadata.data.clone();

    assert_eq!(master_metadata_before.name, child_metadata.name);

    increment_uri(&mut master_metadata_before.uri, is_last_cycle).unwrap();
    unpuff_metadata(&mut master_metadata_before);
    unpuff_metadata(&mut master_metadata_after);

    assert_eq!(master_metadata_before, master_metadata_after);
}

#[tokio::test]
async fn test_process_close_idle_rapid_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

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

    let mut auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    // Close idle cycles until automatically filtered
    for cycle_number in 1..(ALLOWED_CONSECUTIVE_IDLE_CYCLES + 2) {
        assert!(!auction_root_state.status.is_filtered);
        warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

        let balance_change = close_cycle_transaction(
            &mut testbench,
            &auction_cycle_payer,
            auction_id,
            &auction_owner.keypair.pubkey(),
            TokenType::Token,
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(-balance_change as u64, TRANSACTION_FEE);

        // Check if idle cycle streak has been incremented
        auction_root_state = testbench
            .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
            .await
            .unwrap();

        assert_eq!(
            auction_root_state.status.current_idle_cycle_streak,
            cycle_number
        );
    }
    assert!(auction_root_state.status.is_filtered);
}
