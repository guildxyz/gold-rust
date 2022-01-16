#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::{initialize_new_auction, TestUser};

use agsol_common::MaxSerializedLen;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::*;
use agsol_gold_contract::unpuff_metadata;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;

use metaplex_token_metadata::ID as META_ID;

use solana_program::program_option::COption;
use solana_program::pubkey::Pubkey;
use spl_token::state::{Account as TokenAccount, Mint};

const TRANSACTION_FEE: u64 = 5_000;
const AUCTION_CREATION_COST: u64 = 24_067_680 + TRANSACTION_FEE;

#[tokio::test]
async fn test_process_initialize_auction() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await;
    let auction_id = [123_u8; 32];
    let auction_config = AuctionConfig {
        cycle_period: 86400,
        encore_period: 300,
        minimum_bid_amount: 10_000,
        number_of_cycles: Some(10),
    };
    let balance_change = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .unwrap();

    assert_eq!(-balance_change as u64, AUCTION_CREATION_COST);

    // check mint account
    let (master_mint_pubkey, _) =
        Pubkey::find_program_address(&master_mint_seeds(&auction_id), &CONTRACT_ID);
    let (master_edition_pubkey, _) = Pubkey::find_program_address(
        &edition_seeds(&master_mint_pubkey),
        &metaplex_token_metadata::ID,
    );

    let master_mint_data: Mint = testbench
        .client()
        .get_packed_account_data(master_mint_pubkey)
        .await
        .unwrap();

    assert!(master_mint_data.is_initialized);
    assert_eq!(
        master_mint_data.mint_authority,
        COption::Some(master_edition_pubkey)
    );
    assert_eq!(master_mint_data.supply, 1);
    assert_eq!(master_mint_data.decimals, 0);

    // check holding account
    let (master_holding_pubkey, _) =
        Pubkey::find_program_address(&master_holding_seeds(&auction_id), &CONTRACT_ID);
    let master_holding_data: TokenAccount = testbench
        .client()
        .get_packed_account_data(master_holding_pubkey)
        .await
        .unwrap();

    assert_eq!(master_holding_data.amount, 1);

    // check metadata
    let (master_metadata_pubkey, _) =
        Pubkey::find_program_address(&metadata_seeds(&master_mint_pubkey), &META_ID);
    let mut master_metadata = testbench
        .get_and_deserialize_account_data::<metaplex_token_metadata::state::Metadata>(
            &master_metadata_pubkey,
        )
        .await;
    unpuff_metadata(&mut master_metadata.data);
    assert_eq!(master_metadata.data.uri, "uri/1.json");

    // check state account
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);
    let cycle_number_bytes = 1_u64.to_le_bytes();
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &cycle_number_bytes),
        &CONTRACT_ID,
    );

    // Assert length of the root state data
    let auction_root_state_data = testbench.get_account_data(&auction_root_state_pubkey).await;
    assert_eq!(
        auction_root_state_data.len(),
        AuctionRootState::MAX_SERIALIZED_LEN + agsol_gold_contract::EXTRA_ROOT_STATE_BYTES
    );

    // Assert that these accounts can be read
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;

    assert_eq!(
        auction_root_state.auction_config.cycle_period,
        auction_config.cycle_period
    );
    assert_eq!(
        auction_root_state.auction_config.encore_period,
        auction_config.encore_period
    );
    assert!(auction_cycle_state.bid_history.get_last_element().is_none());

    let (auction_pool_pubkey, _) =
        Pubkey::find_program_address(&auction_pool_seeds(), &CONTRACT_ID);
    let auction_pool = testbench
        .get_and_deserialize_account_data::<AuctionPool>(&auction_pool_pubkey)
        .await;
    assert_eq!(1, auction_pool.pool.len());
    assert_eq!(auction_pool.pool[0], [123_u8; 32]);

    // Invalid use case
    // Create auction with the same id
    let reinitialize_auction_error = initialize_new_auction(
        &mut testbench,
        &auction_owner.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        reinitialize_auction_error,
        AuctionContractError::AuctionIdNotUnique
    );

    let other_user = TestUser::new(&mut testbench).await;

    let initialize_auction_with_same_id_error = initialize_new_auction(
        &mut testbench,
        &other_user.keypair,
        &auction_config,
        auction_id,
        TokenType::Nft,
    )
    .await
    .err()
    .unwrap();
    assert_eq!(
        initialize_auction_with_same_id_error,
        AuctionContractError::AuctionIdNotUnique
    );
}
