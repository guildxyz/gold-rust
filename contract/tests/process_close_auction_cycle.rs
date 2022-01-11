#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::processor::increment_uri;
use agsol_gold_contract::state::*;
use agsol_gold_contract::unpuff_metadata;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use metaplex_token_metadata::instruction::CreateMetadataAccountArgs;
use metaplex_token_metadata::state::Metadata;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

const CLOSE_AUCTION_CYCLE_LAST_CYCLE: u64 = 12_799_440;
const CLOSE_AUCTION_CYCLE_COST_EXISTING_MARKER: u64 = 16_613_520;

#[tokio::test]
async fn test_process_close_auction_cycle() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(1000),
    };

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

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

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    let (auction_cycle_state_pubkey, auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey).await;

    // Test no bids were taken
    // Close first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);
    let (same_auction_cycle_state_pubkey, same_auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey).await;

    // Check if auction timings were correctly updated
    assert_eq!(auction_cycle_state_pubkey, same_auction_cycle_state_pubkey);
    assert_eq!(
        auction_cycle_state.start_time,
        same_auction_cycle_state.start_time
    );
    assert_eq!(
        auction_cycle_state.end_time + auction_config.cycle_period,
        same_auction_cycle_state.end_time
    );
    let current_time = testbench.block_time().await;
    assert!(current_time < same_auction_cycle_state.end_time);

    // Check no nft was minted
    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    assert_eq!(
        testbench.get_token_account(&child_edition.holding).await,
        Err("Account not found".to_string())
    );
    assert_eq!(
        testbench.get_mint_account(&child_edition.mint).await,
        Err("Account not found".to_string())
    );

    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;

    // Check if other data are unchanged
    assert_eq!(
        auction_root_state.auction_config.cycle_period,
        auction_config.cycle_period
    );
    assert_eq!(
        auction_root_state.auction_config.encore_period,
        auction_config.encore_period
    );
    assert!(auction_cycle_state.bid_history.get_last_element().is_none());

    // Test some bids were taken
    // Place bid
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;

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

    let (auction_cycle_state_pubkey, _auction_cycle_state) =
        get_auction_cycle_state(&mut testbench, &auction_root_state_pubkey).await;

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

    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;
    assert!(auction_cycle_state.bid_history.get_last_element().is_none());
}

#[tokio::test]
async fn test_ended_close_cycle_on_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(1),
    };

    let user = TestUser::new(&mut testbench).await;
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

    // Place bid
    let user_1 = TestUser::new(&mut testbench).await;
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close first (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

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
        CLOSE_AUCTION_CYCLE_LAST_CYCLE + TRANSACTION_FEE,
    );

    // Invalid use case
    // Bidding on ended auction
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let bid_amount_higher = 2_000_000;
    let bid_on_ended_auction_error =
        place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount_higher)
            .await
            .err()
            .unwrap();

    // Invalid use case
    // Closing cycle of ended auction
    assert_eq!(
        bid_on_ended_auction_error,
        AuctionContractError::AuctionEnded
    );

    let close_cycle_on_ended_auction_error = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        close_cycle_on_ended_auction_error,
        AuctionContractError::AuctionEnded
    );
}

#[tokio::test]
async fn test_close_cycle_on_frozen_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 100_000, // lamports
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
    .unwrap();

    let auction_cycle_payer = TestUser::new(&mut testbench).await.keypair;
    let (auction_root_state_pubkey, _auction_cycle_state_pubkey) =
        get_state_pubkeys(&mut testbench, auction_id).await;

    // Freeze auction
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.status.is_frozen);

    // Invalid use case
    // End cycle on frozen auction

    // Warp to slot so that the cycle could be closed if it was not frozen
    warp_to_cycle_end(&mut testbench, auction_id).await;

    // Trying to close the cycle
    let close_cycle_on_frozen_auction_error = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        close_cycle_on_frozen_auction_error,
        AuctionContractError::AuctionFrozen
    );
}

#[tokio::test]
async fn test_close_cycle_child_metadata_change_not_repeating() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(3),
    };

    let auction_cycle_payer = TestUser::new(&mut testbench).await.keypair;

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    let user_1 = TestUser::new(&mut testbench).await;

    // Place bid on first cycle
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close the first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    let master_metadata_before = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 1);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;
    let child_metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&child_edition.metadata)
        .await;

    check_metadata_update(
        &master_metadata_before,
        &master_metadata_after,
        &child_metadata,
        false,
    );

    // Place bid on second cycle
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close the second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;
    let master_metadata_before = master_metadata_after.clone();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 2);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;
    let child_metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&child_edition.metadata)
        .await;

    check_metadata_update(
        &master_metadata_before,
        &master_metadata_after,
        &child_metadata,
        false,
    );

    // Place bid on last cycle
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close the third (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;
    let master_metadata_before = master_metadata_after.clone();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 3);
    let child_edition = EditionPda::new(EditionType::Child(next_edition), &auction_id);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;
    let child_metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&child_edition.metadata)
        .await;

    check_metadata_update(
        &master_metadata_before,
        &master_metadata_after,
        &child_metadata,
        true,
    );
}

#[tokio::test]
async fn test_child_close_cycle_metadata_change_repeating() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 100_000, // lamports
        number_of_cycles: Some(3),
    };

    let auction_cycle_payer = TestUser::new(&mut testbench).await.keypair;

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let create_token_args = CreateTokenArgs::Nft {
        metadata_args: CreateMetadataAccountArgs {
            data: metaplex_token_metadata::state::Data {
                name: "random auction".to_owned(),
                symbol: "RAND".to_owned(),
                uri: "uri/1.json".to_owned(),
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
    .unwrap();

    // bid to first cycle
    let user_1 = TestUser::new(&mut testbench).await;
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close the first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    let master_metadata_before = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 1);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;

    assert_eq!(
        master_metadata_after.data.name,
        master_metadata_before.data.name
    );
    assert_eq!(
        master_metadata_after.data.uri,
        master_metadata_before.data.uri
    );

    // bid to second cycle
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close the second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;
    let master_metadata_before = master_metadata_after.clone();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 2);

    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;

    assert_eq!(
        master_metadata_after.data.name,
        master_metadata_before.data.name
    );
    assert_eq!(
        master_metadata_after.data.uri,
        master_metadata_before.data.uri
    );

    // bid to the last cycle
    let bid_amount = 10_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap();

    // Close the third (last) cycle
    warp_to_cycle_end(&mut testbench, auction_id).await;

    let next_edition = get_next_child_edition(&mut testbench, &auction_root_state_pubkey).await;

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap();

    // Check minted nft
    assert_eq!(next_edition, 3);
    let master_metadata_after = testbench
        .get_and_deserialize_account_data::<Metadata>(&master_edition.metadata)
        .await;

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
