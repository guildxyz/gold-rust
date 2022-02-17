#![cfg(feature = "test-bpf")]
mod test_factory;
use test_factory::*;

use agsol_common::MaxSerializedLen;
use agsol_gold_contract::instruction::factory::*;
use agsol_gold_contract::pda::*;
use agsol_gold_contract::state::{AuctionConfig, ContractBankState, TokenType};
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_testbench::tokio;
use solana_program::pubkey::Pubkey;
use solana_sdk::signer::Signer;

#[tokio::test]
async fn test_process_admin_withdraw() {
    let (mut testbench, auction_owner) = test_factory::testbench_setup().await.unwrap().unwrap();
    let auction_id = [1; 32];
    let auction_config = AuctionConfig {
        cycle_period: 60,
        encore_period: 1,
        minimum_bid_amount: 50_000_000, // lamports
        number_of_cycles: Some(1000),
    };

    let payer = testbench.clone_payer();

    let user_1 = TestUser::new(&mut testbench).await.unwrap().unwrap();
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
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();
    // Place bid
    let bid_amount = 100_000_000;
    place_bid_transaction(&mut testbench, auction_id, &user_1.keypair, bid_amount)
        .await
        .unwrap()
        .unwrap();

    // Close second cycle
    warp_to_cycle_end(&mut testbench, auction_id).await.unwrap();

    let _balance_change = close_cycle_transaction(
        &mut testbench,
        &auction_cycle_payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        TokenType::Nft,
    )
    .await
    .unwrap()
    .unwrap();

    // claim funds
    let (contract_bank_pubkey, _) =
        Pubkey::find_program_address(&contract_bank_seeds(), &CONTRACT_ID);
    let contract_balance_before = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();

    let claim_amount = 1_000_000;
    let owner_balance_change = claim_funds_transaction(
        &mut testbench,
        &payer,
        auction_id,
        &auction_owner.keypair.pubkey(),
        claim_amount,
    )
    .await
    .unwrap()
    .unwrap();
    let contract_balance_after = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();

    assert_eq!(claim_amount / 20 * 19, owner_balance_change as u64);

    assert_eq!(
        claim_amount - (claim_amount / 20 * 19),
        contract_balance_after - contract_balance_before
    );

    // attempt to withdraw without authority
    let admin_withdraw_args = AdminWithdrawArgs {
        withdraw_authority: user_1.keypair.pubkey(),
        amount: 5000,
    };
    let withdraw_instruction = admin_withdraw(&admin_withdraw_args);
    let error = testbench
        .process_transaction(&[withdraw_instruction], &user_1.keypair, None)
        .await
        .unwrap()
        .err()
        .unwrap();

    assert_eq!(
        to_auction_error(error),
        AuctionContractError::WithdrawAuthorityMismatch
    );

    let contract_bank_state = testbench
        .get_and_deserialize_account_data::<ContractBankState>(&contract_bank_pubkey)
        .await
        .unwrap();
    assert_eq!(
        contract_bank_state.withdraw_authority,
        testbench.payer().pubkey()
    );
    assert_eq!(
        contract_bank_state.contract_admin,
        testbench.payer().pubkey()
    );

    let rent_program = testbench.client().get_rent().await.unwrap();
    let minimum_bank_rent = rent_program.minimum_balance(ContractBankState::MAX_SERIALIZED_LEN);

    let mut admin_withdraw_args = AdminWithdrawArgs {
        withdraw_authority: payer.pubkey(), // payer is the contract admin
        amount: contract_balance_after - minimum_bank_rent + 10, // slightly more than the max allowed amount
    };
    let withdraw_instruction = admin_withdraw(&admin_withdraw_args);
    let error = testbench
        .process_transaction(&[withdraw_instruction], &payer, None)
        .await
        .unwrap()
        .err()
        .unwrap();

    assert_eq!(
        to_auction_error(error),
        AuctionContractError::InvalidClaimAmount
    );

    let withdraw_authority_balance_before = testbench
        .get_account_lamports(&payer.pubkey())
        .await
        .unwrap();

    // withdraw proper amount
    admin_withdraw_args.amount = TRANSACTION_FEE + 100;
    let withdraw_instruction = admin_withdraw(&admin_withdraw_args);
    let result = testbench
        .process_transaction(&[withdraw_instruction.clone()], &payer, None)
        .await
        .unwrap();

    assert!(result.is_ok());

    let withdraw_authority_balance_after = testbench
        .get_account_lamports(&payer.pubkey())
        .await
        .unwrap();
    let contract_bank_balance_after_2 = testbench
        .get_account_lamports(&contract_bank_pubkey)
        .await
        .unwrap();

    assert_eq!(
        withdraw_authority_balance_after - withdraw_authority_balance_before,
        admin_withdraw_args.amount - TRANSACTION_FEE
    );
    assert_eq!(
        contract_balance_after - contract_bank_balance_after_2,
        admin_withdraw_args.amount
    );

    // reassign withdraw authority to user_1
    let reassign_args = AdminWithdrawReassignArgs {
        withdraw_authority: payer.pubkey(),
        new_withdraw_authority: user_1.keypair.pubkey(),
    };
    let reassign_instruction = admin_withdraw_reassign(&reassign_args);
    // NOTE this would panic because the withdraw authority should be a signer whereas it's not
    //let result = testbench
    //    .process_transaction(&[reassign_instruction.clone()], &user_1.keypair, None)
    //    .await;

    // reassign signed by true authority
    let result = testbench
        .process_transaction(&[reassign_instruction], &payer, None)
        .await
        .unwrap();

    assert!(result.is_ok());

    let contract_bank_state = testbench
        .get_and_deserialize_account_data::<ContractBankState>(&contract_bank_pubkey)
        .await
        .unwrap();

    assert_eq!(
        contract_bank_state.withdraw_authority,
        user_1.keypair.pubkey(),
    );

    // attempt to withdraw as old withraw authority
    let error = testbench
        .process_transaction(&[withdraw_instruction], &payer, None)
        .await
        .unwrap()
        .err()
        .unwrap();

    assert_eq!(
        to_auction_error(error),
        AuctionContractError::WithdrawAuthorityMismatch
    );

    // withdraw as new withdraw authority
    admin_withdraw_args.withdraw_authority = user_1.keypair.pubkey();
    let withdraw_instruction = admin_withdraw(&admin_withdraw_args);

    let user_balance_before = testbench
        .get_account_lamports(&user_1.keypair.pubkey())
        .await
        .unwrap();
    let result = testbench
        .process_transaction(&[withdraw_instruction], &user_1.keypair, None)
        .await
        .unwrap();
    assert!(result.is_ok());

    let user_balance_after = testbench
        .get_account_lamports(&user_1.keypair.pubkey())
        .await
        .unwrap();
    assert_eq!(
        user_balance_after - user_balance_before,
        admin_withdraw_args.amount - TRANSACTION_FEE
    );
}
