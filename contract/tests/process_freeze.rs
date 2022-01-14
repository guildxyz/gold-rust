#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;
use solana_sdk::signer::Signer;

const TRANSACTION_FEE: u64 = 5000;

#[tokio::test]
async fn test_process_freeze() {
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
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(!auction_root_state.status.is_frozen);

    // Bid to auction once
    let user = TestUser::new(&mut testbench).await;
    let initial_balance = 150_000_000;
    assert_eq!(
        initial_balance,
        get_account_lamports(&mut testbench, &user.keypair.pubkey()).await
    );

    let bid_amount = 10_000_000;
    let balance_change =
        place_bid_transaction(&mut testbench, auction_id, &user.keypair, bid_amount)
            .await
            .unwrap();

    assert_eq!(-balance_change as u64, bid_amount + TRANSACTION_FEE);

    // Freezing auction
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.status.is_frozen);
    assert_eq!(auction_root_state.all_time_treasury, 0);
    assert_eq!(
        initial_balance - TRANSACTION_FEE,
        get_account_lamports(&mut testbench, &user.keypair.pubkey()).await
    );

    // Freezing already frozen auction
    // NOTE: has no effect
    freeze_auction_transaction(&mut testbench, auction_id, &auction_owner.keypair)
        .await
        .unwrap();
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    assert!(auction_root_state.status.is_frozen);
}
