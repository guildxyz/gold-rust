#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use solana_program::program_option::COption;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

use agsol_gold_contract::instruction::factory::TokenType;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::{tokio, TestbenchError};

// This file includes the following tests:
//
// Valid use cases:
//   - Creating token auction
//   - Creating token auction using existing mint
//   - Bidding on token auction
//   - Closing token auction cycle with and without placed bids
//   - Claiming rewards from token auction non-chronologically
//   - Claiming rewards from token auction initialized with existing mint
//
// Invalid use cases:
//   - Creating token auction with 0 per_cycle_amount

const CLOSE_CYCLE_COST: u64 = 3_758_400;
const CLAIM_REWARDS_COST_TOKEN: u64 = 2_039_280;

#[tokio::test]
async fn test_process_tokens() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_cycle_payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    // Invalid use case
    // Initialize auction with 0 per cycle token amount
    let create_token_args = CreateTokenArgs::Token {
        decimals: 0,
        per_cycle_amount: 0,
        existing_mint: None,
    };
    let invalid_per_cycle_amount_error = initialize_new_auction_custom(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        create_token_args,
    )
    .await
    .unwrap()
    .err()
    .unwrap();

    assert_eq!(
        invalid_per_cycle_amount_error,
        AuctionContractError::InvalidPerCycleAmount,
    );

    // Initialize properly
    initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    let (token_mint_pubkey, _) =
        Pubkey::find_program_address(&token_mint_seeds(&auction_id), &CONTRACT_ID);

    let (contract_pda, _) = Pubkey::find_program_address(&contract_pda_seeds(), &CONTRACT_ID);

    let token_mint = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();

    // Check created mint
    assert_eq!(token_mint.mint_authority, COption::Some(contract_pda));
    assert_eq!(token_mint.supply, 0);
    assert_eq!(token_mint.decimals, 1);
    assert!(token_mint.is_initialized);
    assert_eq!(token_mint.freeze_authority, COption::None);

    // Test no bids were taken
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    // Check that no tokens were minted
    let token_mint = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();
    assert_eq!(token_mint.supply, 0);

    let (token_holding_pubkey, _) = Pubkey::find_program_address(
        &token_holding_seeds(&token_mint_pubkey, &auction_owner.keypair.pubkey()),
        &CONTRACT_ID,
    );
    assert_eq!(
        testbench
            .get_token_account(&token_holding_pubkey)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );

    // Test after a bid was taken
    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();

    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Closing cycle after bid
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    // Check that closing the cycle did not mint any tokens
    let token_mint = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();
    assert_eq!(token_mint.supply, 0);

    assert_eq!(
        testbench
            .get_token_account(&token_holding_pubkey)
            .await
            .err()
            .unwrap(),
        TestbenchError::AccountNotFound
    );
}

#[tokio::test]
async fn test_process_tokens_existing_mint() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let payer = TestUser::new(&mut testbench)
        .await
        .unwrap()
        .unwrap()
        .keypair;
    let user = TestUser::new(&mut testbench).await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let token_mint_pubkey = testbench
        .create_mint(10, &auction_owner.keypair.pubkey())
        .await
        .unwrap()
        .unwrap();

    //  initialize auction with existing mint
    let per_cycle_amount = 100;
    let create_token_args = CreateTokenArgs::Token {
        decimals: 0,
        per_cycle_amount,
        existing_mint: Some(token_mint_pubkey),
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

    let mint_info = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();

    let contract_pda_pubkey = Pubkey::find_program_address(&contract_pda_seeds(), &CONTRACT_ID).0;
    assert_eq!(mint_info.mint_authority.unwrap(), contract_pda_pubkey);

    // Placing bid
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Closing cycle after bid
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    // Check that no tokens were minted after cycle closing
    let token_mint = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();
    assert_eq!(token_mint.supply, 0);

    let (token_holding_pubkey, _) = Pubkey::find_program_address(
        &token_holding_seeds(&token_mint_pubkey, &user.keypair.pubkey()),
        &CONTRACT_ID,
    );
    assert!(!is_existing_account(&mut testbench, &token_holding_pubkey)
        .await
        .unwrap());

    // Claiming token rewards
    let balance_change = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user.keypair.pubkey(),
        1,
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLAIM_REWARDS_COST_TOKEN + TRANSACTION_FEE
    );

    // Check that tokens were minted upon claim
    let token_mint = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();
    assert_eq!(token_mint.supply, per_cycle_amount);

    assert!(is_existing_account(&mut testbench, &token_holding_pubkey)
        .await
        .unwrap());

    let user_token_account = testbench
        .get_token_account(&token_holding_pubkey)
        .await
        .unwrap();
    assert_eq!(user_token_account.amount, per_cycle_amount);
    assert_eq!(user_token_account.owner, user.keypair.pubkey());
}

#[tokio::test]
async fn test_process_claim_rewards_token_non_chronological() {
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
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    // Place bid on first cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close first cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, CLOSE_CYCLE_COST + TRANSACTION_FEE,);

    // Place bid on second cycle
    let bid_amount = 50_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_2.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let balance_change = close_cycle_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(-balance_change as u64, CLOSE_CYCLE_COST + TRANSACTION_FEE,);

    // Check that no tokens have been claimed yet
    let token_data = get_token_data(&mut testbench, &auction_root_state_pubkey)
        .await
        .unwrap()
        .unwrap();
    let token_mint = testbench.get_mint_account(&token_data.mint).await.unwrap();
    assert_eq!(token_mint.supply, 0);

    // Claim rewards from first cycle
    let balance_change = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_1.keypair.pubkey(),
        1,
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLAIM_REWARDS_COST_TOKEN + TRANSACTION_FEE
    );

    // Check if asset holding is created and asset is minted
    let (user_1_holding_pubkey, _) = Pubkey::find_program_address(
        &token_holding_seeds(&token_data.mint, &user_1.keypair.pubkey()),
        &CONTRACT_ID,
    );
    assert!(is_existing_account(&mut testbench, &user_1_holding_pubkey)
        .await
        .unwrap());

    let token_mint = testbench.get_mint_account(&token_data.mint).await.unwrap();
    assert_eq!(token_mint.supply, token_data.per_cycle_amount,);

    // Check that second holding account is not created
    let (user_2_holding_pubkey, _) = Pubkey::find_program_address(
        &token_holding_seeds(&token_data.mint, &user_2.keypair.pubkey()),
        &CONTRACT_ID,
    );
    assert!(!is_existing_account(&mut testbench, &user_2_holding_pubkey)
        .await
        .unwrap());

    // Claim rewards from second cycle
    let balance_change = claim_rewards_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &user_2.keypair.pubkey(),
        2,
        TokenType::Token,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        -balance_change as u64,
        CLAIM_REWARDS_COST_TOKEN + TRANSACTION_FEE
    );

    // Check if asset holding is created and asset is minted
    assert!(is_existing_account(&mut testbench, &user_2_holding_pubkey)
        .await
        .unwrap());

    let token_mint = testbench.get_mint_account(&token_data.mint).await.unwrap();
    assert_eq!(token_mint.supply, 2 * token_data.per_cycle_amount,);
}
