#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;

const TRANSACTION_FEE: u64 = 5000;

#[tokio::test]
async fn test_process_verify_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;

    let auction_id = [2; 32];
    let auction_config = AuctionConfig {
        cycle_period: 100,
        encore_period: 30,
        minimum_bid_amount: 10_000,
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
    .unwrap();

    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(!auction_root_state.is_verified);

    // Verifying auction
    let payer = testbench.clone_payer();
    let balance_change = verify_auction_transaction(&mut testbench, auction_id, &payer)
        .await
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.is_verified);

    assert_eq!(-balance_change as u64, TRANSACTION_FEE);

    // Verifying already Verified auction
    // NOTE: has no effect
    verify_auction_transaction(&mut testbench, auction_id, &payer)
        .await
        .unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.is_verified);
}
