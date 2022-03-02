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

use solana_sdk::signer::Signer;

const CLAIM_REWARDS_COST_NFT: u64 = 12_799_440;

// This file includes the following tests:
//
// Valid use cases:
//   - Claiming rewards from nft auctions
//   - Claiming rewards from nft auctions chronologically and non-chronologically
//   - (Test for claiming rewards from token auctions in `process_tokens.rs`)
//
// Invalid use cases:
//   - Claiming rewards from ongoing cycle with and without placed bets
//   - Claiming rewards using other than the top bidder account
//   - Claiming already claimed rewards

#[tokio::test]
async fn test_process_claim_rewards_nft() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let payer = TestUser::new(&mut testbench)
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

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    // Close first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);

    // Invalid use case
    // Try claiming rewards while no bids were taken
    let claim_reward_from_ongoing_cycle_no_bids_error = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_1.keypair.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        claim_reward_from_ongoing_cycle_no_bids_error,
        AuctionContractError::AuctionIsInProgress
    );

    // Test some bids were taken
    // Place bid
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Invalid use case
    // Try claiming rewards from ongoing cycle
    let claim_reward_from_ongoing_cycle_error = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_1.keypair.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        claim_reward_from_ongoing_cycle_error,
        AuctionContractError::AuctionIsInProgress
    );

    // Close first cycle with bid this time
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

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.unclaimed_rewards, 1);

    // Invalid use case
    // Try claiming rewards to invalid top bidder account
    let claim_reward_invalid_top_bidder_error = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &payer.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        claim_reward_invalid_top_bidder_error,
        AuctionContractError::TopBidderAccountMismatch
    );

    // Claim rewards
    let balance_change = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_1.keypair.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLAIM_REWARDS_COST_NFT + TRANSACTION_FEE
    );

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.unclaimed_rewards, 0);

    // Check if asset holding is created and asset is minted
    let child_edition = EditionPda::new(EditionType::Child(1), &auction_id);
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

    assert_metadata_uri(&mut testbench, &child_edition, "1.json").await;

    // Invalid use case
    // Trying to claim rewards again
    let repeated_claim_reward_error = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_1.keypair.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        repeated_claim_reward_error,
        AuctionContractError::RewardAlreadyClaimed
    );

    // Check master metadata is correct after claim
    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    assert_metadata_uri(&mut testbench, &master_edition, "2.json").await;
}

#[tokio::test]
async fn test_process_claim_rewards_nft_non_chronological() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let user_2 = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let payer = TestUser::new(&mut testbench)
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

    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    // Place bid on first cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close first cycle
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

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.unclaimed_rewards, 1);

    // Place bid on second cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_2.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close second cycle
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

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.unclaimed_rewards, 2);

    // Check that no child nfts have been claimed yet
    let first_child_edition = EditionPda::new(EditionType::Child(1), &auction_id);
    assert!(
        !is_existing_account(&mut testbench, &first_child_edition.mint)
            .await
            .unwrap()
    );

    let second_child_edition = EditionPda::new(EditionType::Child(2), &auction_id);
    assert!(
        !is_existing_account(&mut testbench, &second_child_edition.mint)
            .await
            .unwrap()
    );

    // Claim rewards from second cycle
    claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_2.keypair.pubkey(),
        2,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.unclaimed_rewards, 1);

    // Check if asset holding is created and asset is minted
    let user_2_nft_account = testbench
        .get_token_account(&second_child_edition.holding)
        .await
        .unwrap();
    let child_mint_account = testbench
        .get_mint_account(&second_child_edition.mint)
        .await
        .unwrap();
    assert_eq!(user_2_nft_account.mint, second_child_edition.mint);
    assert_eq!(user_2_nft_account.owner, user_2.keypair.pubkey());
    assert_eq!(user_2_nft_account.amount, 1);
    assert_eq!(child_mint_account.supply, 1);

    // Check that first child nft is still not created
    assert!(
        !is_existing_account(&mut testbench, &first_child_edition.mint)
            .await
            .unwrap()
    );

    // Check child metadata is correct
    let child_edition = EditionPda::new(EditionType::Child(2), &auction_id);
    assert_metadata_uri(&mut testbench, &child_edition, "2.json").await;

    // Check master metadata is correct after claim
    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    assert_metadata_uri(&mut testbench, &master_edition, "3.json").await;

    // Claim rewards from second cycle
    claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_1.keypair.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_root_state.unclaimed_rewards, 0);

    // Check if asset holding is created and asset is minted
    let user_1_nft_account = testbench
        .get_token_account(&first_child_edition.holding)
        .await
        .unwrap();
    let child_mint_account = testbench
        .get_mint_account(&first_child_edition.mint)
        .await
        .unwrap();
    assert_eq!(user_1_nft_account.mint, first_child_edition.mint);
    assert_eq!(user_1_nft_account.owner, user_1.keypair.pubkey());
    assert_eq!(user_1_nft_account.amount, 1);
    assert_eq!(child_mint_account.supply, 1);

    // Check child metadata is correct
    let child_edition = EditionPda::new(EditionType::Child(1), &auction_id);
    assert_metadata_uri(&mut testbench, &child_edition, "1.json").await;

    // Check master metadata is correct after claim
    let master_edition = EditionPda::new(EditionType::Master, &auction_id);
    assert_metadata_uri(&mut testbench, &master_edition, "3.json").await;
}
