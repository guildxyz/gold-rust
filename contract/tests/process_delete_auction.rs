#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;

use agsol_gold_contract::instruction::factory::TokenType;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_gold_contract::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL;

use agsol_testbench::tokio;
use agsol_testbench::Testbench;

use std::str::FromStr;

// This file includes the following tests:
//
// Valid use cases:
//   - Deleting an auction immediately after creating it
//   - Deleting an auction with less than RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL cycles
//   - Deleting an auction with exactly RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL cycles
//   - Deleting an auction with more than RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL cycles
//   - Deleting an auction with its bank deallocated by claiming all its funds
//   - Deleting finished and ongoing auctions
//   - Claiming funds on deleting auctions
//
// Invalid use cases:
//   - Deleting an auction without the owner's signature
//   - Deleting an auction with unclaimed rewards
#[tokio::test]
async fn test_delete_auction_immediately() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
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

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    // Invalid use case
    // Deleting an auction without the owner's signature
    let random_user = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let delete_without_owner_signature_error =
        delete_auction_transaction(&mut testbench, &random_user.keypair, auction_id)
            .await
            .unwrap()
            .err()
            .unwrap();
    assert_eq!(
        delete_without_owner_signature_error,
        AuctionContractError::AuctionOwnerMismatch
    );

    delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
        .await
        .unwrap()
        .unwrap();

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 0);

    // Test if state accounts are deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 1).await);
}

#[tokio::test]
async fn test_delete_auction_with_unclaimed_rewards() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
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

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    let user = TestUser::new(&mut testbench).await.unwrap().unwrap();

    // Close a cycle with a bid
    place_bid_transaction(&mut testbench, auction_id, &user.keypair, 50_000_000)
        .await
        .unwrap()
        .unwrap();

    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    close_cycle_transaction(
        &mut testbench,
        &user.keypair,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Invalid use case
    // Deleting an auction with unclaimed rewards
    let delete_without_owner_signature_error =
        delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
            .await
            .unwrap()
            .err()
            .unwrap();
    assert_eq!(
        delete_without_owner_signature_error,
        AuctionContractError::UnclaimedRewards
    );

    // Claiming the rewards to be able to delete the auction
    claim_rewards_transaction(
        &mut testbench,
        &user.keypair,
        auction_id,
        &user.keypair.pubkey(),
        1,
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // Delete should be successful now
    delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
        .await
        .unwrap()
        .unwrap();

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 0);

    // Test if state accounts are deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 1).await);
}

#[tokio::test]
async fn test_delete_small_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(10),
    };

    let payer = testbench.clone_payer();

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

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_and_claim_n_cycles(&mut testbench, auction_id, &auction_owner, &payer, 3).await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_auction_cycle, 4);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 1);

    delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
        .await
        .unwrap()
        .unwrap();

    // Test if auction was removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert!(auction_pool.pool.is_empty());

    // Test if state accounts are deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 4).await);
}

#[tokio::test]
async fn test_delete_claimed_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let number_of_cycles = Some(3);
    assert!(number_of_cycles.unwrap() < RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL);

    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles,
    };

    let payer = testbench.clone_payer();

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

    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    close_and_claim_n_cycles(&mut testbench, auction_id, &auction_owner, &payer, 3).await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_root_state.status.current_auction_cycle, 3);
    assert!(auction_root_state.status.is_finished);

    // Claim all funds from auction so that the auction bank is deallocated
    let claim_all = testbench
        .get_account_lamports(&auction_bank_pubkey)
        .await
        .unwrap();

    claim_funds_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        claim_all,
    )
    .await
    .unwrap()
    .unwrap();

    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());

    // Delete auction with deallocated bank
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(secondary_pool.pool.len(), 1);

    delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
        .await
        .unwrap()
        .unwrap();

    // Test if auction was removed from the pool
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert!(secondary_pool.pool.is_empty());

    // Test if state accounts are deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 3).await);
}

#[tokio::test]
async fn test_delete_just_long_enough_finished_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL),
    };

    let payer = testbench.clone_payer();

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

    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &CONTRACT_ID);

    close_and_claim_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    assert!(auction_root_state.status.is_finished);

    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &1_u64.to_le_bytes()),
        &CONTRACT_ID,
    );

    let auction_bank_balance = testbench
        .get_account_lamports(&auction_bank_pubkey)
        .await
        .unwrap();
    let auction_cycle_balance_sum = 30
        * testbench
            .get_account_lamports(&auction_cycle_state_pubkey)
            .await
            .unwrap();
    let auction_root_balance = testbench
        .get_account_lamports(&auction_root_state_pubkey)
        .await
        .unwrap();

    // Delete auction
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(secondary_pool.pool.len(), 1);

    let contract_balance_before = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();

    let owner_balance_change =
        delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
            .await
            .unwrap()
            .unwrap();

    let contract_balance_after = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();

    // Test that auction is removed from the pool
    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();
    assert!(secondary_pool.pool.is_empty());

    // Test that state accounts are also deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(
        are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 30).await
    );

    // Test that all state balances are claimed correctly
    let fee_multiplier = get_protocol_fee_multiplier(&mut testbench).await;
    let protocol_fee = (auction_bank_balance as f64 * fee_multiplier) as u64;
    assert_eq!(
        protocol_fee + auction_cycle_balance_sum,
        contract_balance_after - contract_balance_before
    );
    assert_eq!(
        auction_bank_balance - protocol_fee + auction_root_balance - TRANSACTION_FEE,
        owner_balance_change as u64
    );
}

#[tokio::test]
async fn test_delete_long_ongoing_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 0,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL + 2),
    };

    let payer = testbench.clone_payer();

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

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    // There will be RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL closed
    // plus one active cycle on chain so it needs two instruction calls
    close_and_claim_n_cycles(
        &mut testbench,
        auction_id,
        &auction_owner,
        &payer,
        RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    )
    .await;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();

    // Assert that the auction is still ongoing
    assert!(!auction_root_state.status.is_finished);

    // Delete auction
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 1);

    delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
        .await
        .unwrap()
        .unwrap();

    // Test that auction is not yet removed from the pool
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert_eq!(auction_pool.pool.len(), 1); // should still be present

    // Test that state accounts are not deleted
    assert!(
        is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(
        are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 2, 31).await
    );
    assert!(does_nth_cycle_state_exist(&mut testbench, &auction_root_state_pubkey, 1).await);

    // Check that auction is inactivated
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await
        .unwrap();
    assert!(auction_root_state.status.is_frozen);

    // Finish deleting the auction
    delete_auction_transaction(&mut testbench, &auction_owner.keypair, auction_id)
        .await
        .unwrap()
        .unwrap();

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();
    assert!(auction_pool.pool.is_empty()); // should be deleted now

    // Test that state accounts are now deleted
    assert!(
        !is_existing_account(&mut testbench, &auction_root_state_pubkey)
            .await
            .unwrap()
    );
    assert!(!is_existing_account(&mut testbench, &auction_bank_pubkey)
        .await
        .unwrap());
    assert!(are_given_cycle_states_deleted(&mut testbench, &auction_root_state_pubkey, 1, 1).await);
}

async fn does_nth_cycle_state_exist(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
    n: u64,
) -> bool {
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(auction_root_state_pubkey, &n.to_le_bytes()),
        &CONTRACT_ID,
    );
    is_existing_account(testbench, &auction_cycle_state_pubkey)
        .await
        .unwrap()
}

async fn are_given_cycle_states_deleted(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
    from: u64,
    to: u64,
) -> bool {
    for i in from..=to {
        if does_nth_cycle_state_exist(testbench, auction_root_state_pubkey, i).await {
            return false;
        }
    }
    true
}

async fn close_and_claim_n_cycles(
    testbench: &mut Testbench,
    auction_id: AuctionId,
    auction_owner: &TestUser,
    payer: &Keypair,
    n: u64,
) {
    for i in 0..n {
        place_bid_transaction(testbench, auction_id, payer, 50_000_000)
            .await
            .unwrap()
            .unwrap();

        warp_to_cycle_end(testbench, auction_id).await.unwrap();

        close_cycle_transaction(
            testbench,
            payer,
            auction_id,
            &auction_owner.keypair.pubkey(),
            TokenType::Nft,
        )
        .await
        .unwrap()
        .unwrap();

        claim_rewards_transaction(
            testbench,
            payer,
            auction_id,
            &payer.pubkey(),
            i + 1,
            TokenType::Nft,
        )
        .await
        .unwrap()
        .unwrap();
    }
}

pub fn increment_uri(uri: &mut String, is_last_cycle: bool) -> Result<(), AuctionContractError> {
    let uri_len = uri.len();
    let mut last_pos = uri_len;
    let mut dot_pos = uri_len;
    let mut slash_pos = uri_len;

    let str_bytes = uri.as_bytes();
    for i in (0..uri_len).rev() {
        if str_bytes[i] == 0 {
            last_pos = i;
        }

        // ".".as_bytes() == [46]
        if str_bytes[i] == 46 {
            dot_pos = i;
        }

        // "/".as_bytes() == [47]
        if str_bytes[i] == 47 {
            slash_pos = i + 1;
            break;
        }
    }

    if last_pos == 0 || dot_pos == 0 || slash_pos == 0 || dot_pos < slash_pos {
        return Err(AuctionContractError::MetadataManipulationError);
    }

    let integer = u64::from_str(&uri[slash_pos..dot_pos])
        .map_err(|_| AuctionContractError::MetadataManipulationError)?;
    uri.truncate(last_pos);
    if is_last_cycle {
        uri.replace_range(slash_pos..dot_pos, &0.to_string());
    } else {
        let incremented_integer = integer
            .checked_add(1)
            .ok_or(AuctionContractError::ArithmeticError)?;
        uri.replace_range(slash_pos..dot_pos, &(incremented_integer).to_string());
    };

    Ok(())
}
