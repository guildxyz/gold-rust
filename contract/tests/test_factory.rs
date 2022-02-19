#![cfg(feature = "test-bpf")]
#![allow(dead_code)]

use num_traits::FromPrimitive;
use std::convert::TryInto;

use agsol_token_metadata::state::MasterEditionV2;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction;
use solana_sdk::instruction::InstructionError;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::TransactionError;

use agsol_gold_contract::instruction::factory::*;
use agsol_gold_contract::pda::{
    auction_bank_seeds, auction_cycle_state_seeds, auction_root_state_seeds,
    protocol_fee_state_seeds, EditionPda,
};
use agsol_gold_contract::state::{
    AuctionConfig, AuctionCycleState, AuctionDescription, AuctionRootState, BidData,
    CreateTokenArgs, ModifyAuctionData, NftData, ProtocolFeeState, TokenConfig, TokenData,
    TokenType,
};
use agsol_gold_contract::utils::unpuff_metadata;
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_gold_contract::{DEFAULT_PROTOCOL_FEE, RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL};

use agsol_common::MaxLenString;
use agsol_testbench::solana_program_test::{self, processor};
use agsol_testbench::{
    Testbench, TestbenchError, TestbenchProgram, TestbenchResult, TestbenchTransactionResult,
};
use agsol_token_metadata::state::Metadata;

pub const TRANSACTION_FEE: u64 = 5000;
pub const INITIAL_AUCTION_POOL_LEN: u32 = 3;

pub fn to_auction_error(program_err: TransactionError) -> AuctionContractError {
    match program_err {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => {
            FromPrimitive::from_u32(code).unwrap()
        }
        //_ => unimplemented!(),
        _ => {
            dbg!(program_err);
            AuctionContractError::InvalidAccountOwner
            //unimplemented!();
        }
    }
}

type TestbenchResultOption<T> = TestbenchResult<Option<T>>;
type AuctionTransactionResult = TestbenchResult<Result<i64, AuctionContractError>>;

pub struct TestContractConfig {
    pub auction_owner: TestUser,
    pub auction_id: [u8; 32],
}

pub struct TestUser {
    pub keypair: Keypair,
}

impl TestUser {
    pub async fn new(testbench: &mut Testbench) -> TestbenchTransactionResult<Self> {
        let keypair = Keypair::new();

        // send lamports to user
        let instruction = system_instruction::transfer(
            &testbench.payer().pubkey(),
            &keypair.pubkey(),
            15_500_000_000,
        );

        let payer = testbench.clone_payer();

        testbench
            .process_transaction(&[instruction], &payer, None)
            .await
            .map(|transaction_result| transaction_result.map(|_| Self { keypair }))
    }
}

pub async fn warp_to_cycle_end(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
) -> TestbenchResult<()> {
    let (_, auction_cycle_state_pubkey) = get_state_pubkeys(testbench, auction_id).await?;
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await?;

    let current_time = testbench.block_time().await?;
    let warp_duration = auction_cycle_state.end_time - current_time + 1;

    if warp_duration > 1 {
        testbench.warp_n_seconds(warp_duration).await?;
    }

    let current_time = testbench.block_time().await?;
    assert!(auction_cycle_state.end_time < current_time);

    Ok(())
}

pub async fn get_next_child_edition(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> TestbenchResultOption<u64> {
    let nft_data = get_nft_data(testbench, auction_root_state_pubkey).await?;

    match nft_data {
        Some(data) => {
            let master_edition_data = testbench
                .get_and_deserialize_account_data::<MasterEditionV2>(&data.master_edition)
                .await?;
            Ok(Some(master_edition_data.supply + 1))
        }
        None => Ok(None),
    }
}

pub async fn get_nft_data(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> TestbenchResultOption<NftData> {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await?;
    match auction_root_state.token_config {
        TokenConfig::Token(_) => Ok(None),
        TokenConfig::Nft(nft_data) => Ok(Some(nft_data)),
    }
}

pub async fn get_token_data(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> TestbenchResultOption<TokenData> {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await?;
    match auction_root_state.token_config {
        TokenConfig::Token(token_data) => Ok(Some(token_data)),
        TokenConfig::Nft(_) => Ok(None),
    }
}

pub async fn get_current_cycle_number(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> TestbenchResult<u64> {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await?;
    Ok(auction_root_state.status.current_auction_cycle)
}

pub async fn get_top_bid(
    testbench: &mut Testbench,
    auction_cycle_state_pubkey: &Pubkey,
) -> TestbenchResultOption<BidData> {
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(auction_cycle_state_pubkey)
        .await?;
    Ok(auction_cycle_state.bid_history.get_last_element().cloned())
}

pub async fn get_bank_balance_without_rent(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
) -> TestbenchResult<u64> {
    let (auction_bank_pubkey, _) =
        Pubkey::find_program_address(&auction_bank_seeds(&auction_id), &CONTRACT_ID);

    let (auction_root_state_pubkey, _auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await?;

    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await?;

    let mut auction_bank_lamports = testbench.get_account_lamports(&auction_bank_pubkey).await?;

    if !auction_root_state.status.is_finished {
        auction_bank_lamports = auction_bank_lamports
            .checked_sub(testbench.rent.minimum_balance(0))
            .ok_or(TestbenchError::RentError)?;
    }
    Ok(auction_bank_lamports)
}

pub async fn get_top_bidder_pubkey(
    testbench: &mut Testbench,
    auction_cycle_state_pubkey: &Pubkey,
) -> TestbenchResultOption<Pubkey> {
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(auction_cycle_state_pubkey)
        .await?;
    Ok(auction_cycle_state
        .bid_history
        .get_last_element()
        .map(|bid_data| bid_data.bidder_pubkey))
}

pub async fn get_state_pubkeys(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
) -> TestbenchResult<(Pubkey, Pubkey)> {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let cycle_number = get_current_cycle_number(testbench, &auction_root_state_pubkey).await?;
    let cycle_number_bytes = cycle_number.to_le_bytes();
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(&auction_root_state_pubkey, &cycle_number_bytes),
        &CONTRACT_ID,
    );

    Ok((auction_root_state_pubkey, auction_cycle_state_pubkey))
}

pub async fn close_cycle_transaction(
    testbench: &mut Testbench,
    payer_keypair: &Keypair,
    auction_id: [u8; 32],
    auction_owner_pubkey: &Pubkey,
    token_type: TokenType,
) -> AuctionTransactionResult {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await?;

    let next_cycle_num = get_current_cycle_number(testbench, &auction_root_state_pubkey).await?;

    let existing_token_mint = match token_type {
        TokenType::Token => {
            let token_data = get_token_data(testbench, &auction_root_state_pubkey)
                .await?
                .ok_or(TestbenchError::AccountNotFound)?;
            Some(token_data.mint)
        }
        TokenType::Nft => None,
    };

    let close_auction_cycle_args = CloseAuctionCycleArgs {
        payer_pubkey: payer_keypair.pubkey(),
        auction_owner_pubkey: *auction_owner_pubkey,
        top_bidder_pubkey: get_top_bidder_pubkey(testbench, &auction_cycle_state_pubkey).await?,
        auction_id,
        next_cycle_num,
        token_type,
        existing_token_mint,
    };

    let close_auction_cycle_ix = close_auction_cycle(&close_auction_cycle_args);

    testbench
        .process_transaction(&[close_auction_cycle_ix], payer_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn delete_auction_transaction(
    testbench: &mut Testbench,
    auction_owner_keypair: &Keypair,
    auction_id: [u8; 32],
) -> AuctionTransactionResult {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await?;

    let current_auction_cycle =
        get_current_cycle_number(testbench, &auction_root_state_pubkey).await?;

    let delete_auction_args = DeleteAuctionArgs {
        auction_owner_pubkey: auction_owner_keypair.pubkey(),
        top_bidder_pubkey: get_top_bidder_pubkey(testbench, &auction_cycle_state_pubkey).await?,
        auction_id,
        current_auction_cycle,
        num_of_cycles_to_delete: RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    };
    let delete_auction_ix = delete_auction(&delete_auction_args);

    testbench
        .process_transaction(&[delete_auction_ix], auction_owner_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn filter_auction_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    filter: bool,
    contract_admin_keypair: &Keypair,
) -> AuctionTransactionResult {
    let filter_instruction = filter_auction(contract_admin_keypair.pubkey(), auction_id, filter);
    testbench
        .process_transaction(&[filter_instruction], contract_admin_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn verify_auction_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    contract_admin_keypair: &Keypair,
) -> AuctionTransactionResult {
    let verify_args = VerifyAuctionArgs {
        contract_admin_pubkey: contract_admin_keypair.pubkey(),
        auction_id,
    };
    let verify_instruction = verify_auction(&verify_args);

    testbench
        .process_transaction(&[verify_instruction], contract_admin_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn set_protocol_fee_transaction(
    testbench: &mut Testbench,
    contract_admin_keypair: &Keypair,
    new_fee: u8,
) -> AuctionTransactionResult {
    let set_fee_args = SetProtocolFeeArgs {
        contract_admin_pubkey: contract_admin_keypair.pubkey(),
        new_fee,
    };
    let set_fee_ix = set_protocol_fee(&set_fee_args);

    testbench
        .process_transaction(&[set_fee_ix], contract_admin_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn modify_auction_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    auction_owner_keypair: &Keypair,
    modify_data: ModifyAuctionData,
) -> AuctionTransactionResult {
    let modify_args = ModifyAuctionArgs {
        auction_owner_pubkey: auction_owner_keypair.pubkey(),
        auction_id,
        modify_data,
    };
    let modify_instruction = modify_auction(&modify_args);

    testbench
        .process_transaction(&[modify_instruction], auction_owner_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn get_protocol_fee_multiplier(testbench: &mut Testbench) -> f64 {
    let (protocol_fee_state_pubkey, _) =
        Pubkey::find_program_address(&protocol_fee_state_seeds(), &CONTRACT_ID);

    let fee_state = testbench
        .get_and_deserialize_account_data::<ProtocolFeeState>(&protocol_fee_state_pubkey)
        .await
        .unwrap_or(ProtocolFeeState {
            fee: DEFAULT_PROTOCOL_FEE,
        });
    fee_state.fee as f64 / 1_000.0
}

pub async fn claim_and_assert_split(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    auction_owner_pubkey: &Pubkey,
    claim_amount: u64,
    contract_bank_pubkey: &Pubkey,
    protocol_fee_state_pubkey: &Pubkey,
    expected_split: u8,
) {
    let contract_balance_before = testbench
        .get_account_lamports(contract_bank_pubkey)
        .await
        .unwrap();
    let payer = testbench.clone_payer();
    let owner_balance_change = claim_funds_transaction(
        testbench,
        &payer,
        auction_id,
        auction_owner_pubkey,
        claim_amount,
    )
    .await
    .unwrap()
    .unwrap();
    let contract_balance_after = testbench
        .get_account_lamports(contract_bank_pubkey)
        .await
        .unwrap();

    let fee_state = testbench
        .get_and_deserialize_account_data::<ProtocolFeeState>(protocol_fee_state_pubkey)
        .await
        .unwrap_or(ProtocolFeeState {
            fee: DEFAULT_PROTOCOL_FEE,
        });

    assert_eq!(expected_split, fee_state.fee);

    let fee_float = fee_state.fee as f64 / 1_000.0;
    let protocol_fee = claim_amount as f64 * fee_float;

    assert_eq!(
        claim_amount - protocol_fee as u64,
        owner_balance_change as u64
    );
    assert_eq!(
        protocol_fee as u64,
        contract_balance_after - contract_balance_before
    );
}

pub async fn claim_funds_transaction(
    testbench: &mut Testbench,
    payer_keypair: &Keypair,
    auction_id: [u8; 32],
    auction_owner_pubkey: &Pubkey,
    amount: u64,
) -> AuctionTransactionResult {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let claim_funds_args = ClaimFundsArgs {
        payer_pubkey: payer_keypair.pubkey(),
        auction_owner_pubkey: *auction_owner_pubkey,
        auction_id,
        cycle_number: get_current_cycle_number(testbench, &auction_root_state_pubkey).await?,
        amount,
    };

    let claim_funds_ix = claim_funds(&claim_funds_args);

    let owner_balance_before = testbench.get_account_lamports(auction_owner_pubkey).await?;

    let testbench_result = testbench
        .process_transaction(&[claim_funds_ix], payer_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error));

    let owner_balance_after = testbench.get_account_lamports(auction_owner_pubkey).await?;
    let owner_balance_change = owner_balance_after as i64 - owner_balance_before as i64;

    testbench_result.map(|transaction_result| {
        transaction_result.map(|_signer_balance_change| owner_balance_change)
    })
}

pub async fn claim_rewards_transaction(
    testbench: &mut Testbench,
    payer_keypair: &Keypair,
    auction_id: [u8; 32],
    top_bidder_pubkey: &Pubkey,
    cycle_number: u64,
    token_type: TokenType,
) -> AuctionTransactionResult {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let existing_token_mint = match token_type {
        TokenType::Token => {
            let token_data = get_token_data(testbench, &auction_root_state_pubkey)
                .await?
                .ok_or(TestbenchError::AccountNotFound)?;
            Some(token_data.mint)
        }
        TokenType::Nft => None,
    };

    let claim_rewards_args = ClaimRewardsArgs {
        payer_pubkey: payer_keypair.pubkey(),
        top_bidder_pubkey: *top_bidder_pubkey,
        auction_id,
        cycle_number,
        token_type,
        existing_token_mint,
    };

    let claim_rewards_ix = claim_rewards(&claim_rewards_args);

    testbench
        .process_transaction(&[claim_rewards_ix], payer_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn assert_metadata_uri(
    testbench: &mut Testbench,
    edition_pda: &EditionPda,
    expected_uri_ending: &str,
) {
    let mut metadata = testbench
        .get_and_deserialize_account_data::<Metadata>(&edition_pda.metadata)
        .await
        .unwrap();
    unpuff_metadata(&mut metadata.data);
    assert!(metadata.data.uri.ends_with(expected_uri_ending));
}

pub async fn place_bid_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    user_keypair: &Keypair,
    amount: u64,
) -> AuctionTransactionResult {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await?;

    let place_bid_args = PlaceBidArgs {
        bidder_pubkey: user_keypair.pubkey(),
        auction_id,
        cycle_number: get_current_cycle_number(testbench, &auction_root_state_pubkey).await?,
        top_bidder_pubkey: get_top_bidder_pubkey(testbench, &auction_cycle_state_pubkey).await?,
        amount,
    };
    let bid_instruction = place_bid(&place_bid_args);

    testbench
        .process_transaction(&[bid_instruction], user_keypair, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn initialize_new_auction_custom(
    testbench: &mut Testbench,
    auction_owner: &Keypair,
    auction_config: &AuctionConfig,
    auction_id: [u8; 32],
    create_token_args: CreateTokenArgs,
) -> AuctionTransactionResult {
    let initialize_auction_args = InitializeAuctionArgs {
        auction_owner_pubkey: auction_owner.pubkey(),
        auction_id,
        auction_config: *auction_config,
        auction_name: auction_id,
        auction_description: AuctionDescription {
            description: MaxLenString::try_from("Cool description").unwrap(),
            socials: vec![MaxLenString::try_from("https://www.gold.xyz").unwrap()]
                .try_into()
                .unwrap(),
            goal_treasury_amount: Some(420_000_000_000),
        },
        create_token_args,
        auction_start_timestamp: None,
    };
    let instruction = initialize_auction(&initialize_auction_args);

    testbench
        .process_transaction(&[instruction], auction_owner, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn initialize_new_auction(
    testbench: &mut Testbench,
    auction_owner: &Keypair,
    auction_config: &AuctionConfig,
    auction_id: [u8; 32],
    token_type: TokenType,
) -> AuctionTransactionResult {
    let initialize_auction_args = InitializeAuctionArgs::new_test(
        auction_owner.pubkey(),
        *auction_config,
        auction_id,
        token_type,
    );
    let instruction = initialize_auction(&initialize_auction_args);

    testbench
        .process_transaction(&[instruction], auction_owner, None)
        .await
        .map(|transaction_result| transaction_result.map_err(to_auction_error))
}

pub async fn get_auction_cycle_pubkey(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> TestbenchResult<Pubkey> {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await?;

    let cycle_number_bytes = auction_root_state
        .status
        .current_auction_cycle
        .to_le_bytes();
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &auction_cycle_state_seeds(auction_root_state_pubkey, &cycle_number_bytes),
        &CONTRACT_ID,
    );

    Ok(auction_cycle_state_pubkey)
}

pub async fn is_existing_account(
    testbench: &mut Testbench,
    account_pubkey: &Pubkey,
) -> TestbenchResult<bool> {
    let account_query = testbench.get_account(account_pubkey).await;
    match account_query {
        Err(err) => Err(err),
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
    }
}

pub async fn get_auction_cycle_state(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> TestbenchResult<(Pubkey, AuctionCycleState)> {
    let auction_cycle_state_pubkey =
        get_auction_cycle_pubkey(testbench, auction_root_state_pubkey).await?;
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await?;

    Ok((auction_cycle_state_pubkey, auction_cycle_state))
}

pub async fn testbench_setup() -> TestbenchTransactionResult<(Testbench, TestUser)> {
    let program_id = agsol_gold_contract::id();
    let testbench_program = TestbenchProgram {
        name: "agsol_gold_contract",
        id: program_id,
        process_instruction: processor!(agsol_gold_contract::processor::process),
    };

    // load metadata program binary
    let meta_program_id = agsol_token_metadata::id();
    let meta_program = TestbenchProgram {
        name: "spl_token_metadata",
        id: meta_program_id,
        process_instruction: None,
    };

    let mut testbench = Testbench::new(&[testbench_program, meta_program]).await?;
    let initialize_contract_args = InitializeContractArgs {
        contract_admin: testbench.payer().pubkey(),
        withdraw_authority: testbench.payer().pubkey(),
        initial_auction_pool_len: INITIAL_AUCTION_POOL_LEN,
    };
    let init_contract_ix = initialize_contract(&initialize_contract_args);
    let result = testbench
        .process_transaction(&[init_contract_ix], &testbench.clone_payer(), None)
        .await;

    // TODO: unwrap here is is somewhat ok because it does not include own contract code
    //  However, this is includes a second process_transaction call with potentially
    //  different transaction error
    let auction_owner = TestUser::new(&mut testbench).await?.unwrap();

    result.map(|transaction_result| transaction_result.map(|_| (testbench, auction_owner)))
}
