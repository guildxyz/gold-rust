#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_gold_contract::instruction::factory::{pool_cleanup, reallocate_pool, AccountToClean};
use agsol_gold_contract::pda::*;
use agsol_gold_contract::solana_program::pubkey::Pubkey;
use agsol_gold_contract::state::{AuctionConfig, AuctionPool, AuctionRootState, TokenType};
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_sdk::signature::Signer;

#[tokio::test]
async fn test_pool_cleanup() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let (secondary_pool_pubkey, _) =
        Pubkey::find_program_address(&secondary_pool_seeds(), &CONTRACT_ID);

    let bot = TestUser::new(&mut testbench).await.unwrap().unwrap();
    let payer = testbench.clone_payer();

    // reallocate auction pools
    let delta = 50_u32;
    let mut new_max_len = 0_u32;
    for _ in 0..4 {
        new_max_len += delta;
        let reallocate_ap_instruction =
            reallocate_pool(&payer.pubkey(), new_max_len, auction_pool_seeds);
        let reallocate_sp_instruction =
            reallocate_pool(&payer.pubkey(), new_max_len, secondary_pool_seeds);
        testbench
            .process_transaction(
                &[reallocate_ap_instruction, reallocate_sp_instruction],
                &payer,
                None,
            )
            .await
            .unwrap()
            .unwrap();
    }

    let mut auction_id = [0_u8; 32];
    let auction_config = AuctionConfig {
        cycle_period: 100,
        encore_period: 30,
        minimum_bid_amount: 50_000_000,
        number_of_cycles: Some(10),
    };

    // fill up pool with 1000 auctions
    // and filter them immediately
    for i in 0..4_usize {
        for j in 1..=(delta as u8) {
            auction_id[i] = j;
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

            filter_auction_transaction(&mut testbench, auction_id, true, &payer)
                .await
                .unwrap()
                .unwrap();

            let seeds = auction_root_state_seeds(&auction_id);
            let (root_state_pubkey, _) = Pubkey::find_program_address(&seeds, &CONTRACT_ID);
            let auction_state = testbench
                .get_and_deserialize_account_data::<AuctionRootState>(&root_state_pubkey)
                .await
                .unwrap();
            assert!(auction_state.status.is_filtered);
        }
    }

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_pool.pool.len() as u32, new_max_len);

    let mut index = 0_usize;
    for _ in 0..10 {
        let mut auctions_to_clean = Vec::with_capacity(20);
        for _ in 0..20 {
            let auction_id = auction_pool.pool[index];
            let seeds = auction_root_state_seeds(&auction_id);
            let (root_state_pubkey, _) = Pubkey::find_program_address(&seeds, &CONTRACT_ID);
            auctions_to_clean.push(AccountToClean {
                pubkey: root_state_pubkey,
                id: auction_id,
            });
            index += 1;
        }
        let clean_ix = pool_cleanup(&bot.keypair.pubkey(), auctions_to_clean);
        testbench
            .process_transaction(&[clean_ix], &bot.keypair, None)
            .await
            .unwrap()
            .unwrap();
    }

    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await
        .unwrap();

    assert_eq!(auction_pool.pool.len(), 0);

    let secondary_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&secondary_pool_pubkey)
        .await
        .unwrap();

    assert_eq!(secondary_pool.pool.len() as u32, new_max_len);
}
