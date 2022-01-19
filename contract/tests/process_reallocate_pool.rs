#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::instruction::factory::reallocate_pool;
use agsol_gold_contract::pda::auction_pool_seeds;
use agsol_gold_contract::state::{AuctionConfig, AuctionPool, TokenType};
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;
use solana_sdk::signature::Signer;

#[tokio::test]
async fn test_process_reallocate_pool() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;
    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;

    assert_eq!(auction_pool.max_len, INITIAL_AUCTION_POOL_LEN);
    assert!(auction_pool.pool.is_empty());

    let mut auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 20,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let payer = testbench.clone_payer();

    for _ in 0..INITIAL_AUCTION_POOL_LEN {
        initialize_new_auction(
            &mut testbench,
            &auction_owner.keypair,
            &auction_config,
            auction_id,
            TokenType::Nft,
        )
        .await
        .unwrap();
        auction_id[0] += 1;
    }

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;

    assert_eq!(auction_pool.pool.len(), INITIAL_AUCTION_POOL_LEN as usize);

    // try to initialize an auction with a full pool
    let result = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await;
    assert_eq!(result, Err(AuctionContractError::AuctionPoolFull));

    let new_max_len = 10_u32;
    let rent_program = testbench.client().get_rent().await.unwrap();
    let old_pool_rent = rent_program.minimum_balance(
        AuctionPool::max_serialized_len(INITIAL_AUCTION_POOL_LEN as usize).unwrap(),
    );
    let new_pool_rent = rent_program
        .minimum_balance(AuctionPool::max_serialized_len(new_max_len as usize).unwrap());
    let admin_balance_before = testbench.get_account_lamports(&payer.pubkey()).await;

    let reallocate_instruction = reallocate_pool(&payer.pubkey(), new_max_len);
    testbench
        .process_transaction(&[reallocate_instruction], &payer, None)
        .await
        .unwrap();

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(auction_pool.pool.len(), INITIAL_AUCTION_POOL_LEN as usize);
    assert_eq!(auction_pool.max_len, new_max_len);
    let admin_balance_after = testbench.get_account_lamports(&payer.pubkey()).await;
    let pool_balance = testbench.get_account_lamports(&auction_pool_pubkey).await;
    let pool_data_len = testbench.get_account_data(&auction_pool_pubkey).await.len();
    assert_eq!(
        admin_balance_before - admin_balance_after,
        TRANSACTION_FEE + new_pool_rent - old_pool_rent
    );
    assert!(rent_program.is_exempt(pool_balance, pool_data_len));
    let result = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await;
    assert!(result.is_ok());
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(
        auction_pool.pool.len(),
        INITIAL_AUCTION_POOL_LEN as usize + 1
    );
    assert_eq!(auction_pool.max_len, new_max_len);

    // try to deallocate/reallocate without admin authority
    let reallocate_instruction = reallocate_pool(&auction_owner.keypair.pubkey(), 0);
    let error = testbench
        .process_transaction(&[reallocate_instruction], &auction_owner.keypair, None)
        .await
        .err()
        .unwrap();
    assert_eq!(
        to_auction_error(error),
        AuctionContractError::ContractAdminMismatch
    );

    // try to shrink the pool, sending these together is fine now,
    // because the size check is before the system program is called
    let reallocate_instruction = reallocate_pool(&payer.pubkey(), 1);
    let error = testbench
        .process_transaction(&[reallocate_instruction], &payer, None)
        .await
        .err()
        .unwrap();
    assert_eq!(
        to_auction_error(error),
        AuctionContractError::ShrinkingPoolIsNotAllowed
    );

    // try to reallocate to a too large size
    let reallocate_instruction = reallocate_pool(&payer.pubkey(), 350_000);
    let result = testbench
        .process_transaction(&[reallocate_instruction], &payer, None)
        .await;
    assert!(result.is_err());
}
