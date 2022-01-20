#![cfg(feature = "test-bpf")]
mod test_factory;

use test_factory::*;

use solana_program::program_option::COption;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::Signer;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::{tokio, TestbenchError};

#[tokio::test]
async fn test_process_tokens() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

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

    assert_eq!(token_mint.mint_authority, COption::Some(contract_pda),);

    assert_eq!(token_mint.supply, 0);

    assert_eq!(token_mint.decimals, 1);

    assert!(token_mint.is_initialized);

    assert_eq!(token_mint.freeze_authority, COption::None,);

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

    let token_mint = testbench
        .get_mint_account(&token_mint_pubkey)
        .await
        .unwrap();
    assert_eq!(token_mint.supply, 100);

    let (token_holding_pubkey, _) = Pubkey::find_program_address(
        &token_holding_seeds(&token_mint_pubkey, &user_1.keypair.pubkey()),
        &CONTRACT_ID,
    );
    let token_holding = testbench
        .get_token_account(&token_holding_pubkey)
        .await
        .unwrap();
    assert_eq!(token_holding.amount, 100);
    assert_eq!(token_holding.owner, user_1.keypair.pubkey());
}
