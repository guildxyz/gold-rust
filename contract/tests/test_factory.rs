#![cfg(feature = "test-bpf")]

use num_traits::FromPrimitive;
use std::convert::TryInto;

use metaplex_token_metadata::state::MasterEditionV2;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction;
use solana_sdk::instruction::InstructionError;
use solana_sdk::signer::keypair::Keypair;
use solana_sdk::signer::Signer;
use solana_sdk::transaction::TransactionError;

use agsol_gold_contract::instruction::factory::*;
use agsol_gold_contract::pda::{get_auction_cycle_state_seeds, get_auction_root_state_seeds};
use agsol_gold_contract::state::{
    AuctionConfig, AuctionCycleState, AuctionDescription, AuctionRootState, BidData,
    CreateTokenArgs, NftData, TokenConfig, TokenData, TokenType,
};
use agsol_gold_contract::AuctionContractError;
use agsol_gold_contract::ID as CONTRACT_ID;
use agsol_gold_contract::RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL;

use agsol_common::MaxLenString;
use agsol_testbench::solana_program_test::{self, processor};
use agsol_testbench::{Testbench, TestbenchProgram};

#[allow(unused)]
pub const TRANSACTION_FEE: u64 = 5000;

// For some reason the compiler always throws dead_code on this
#[allow(dead_code)]
pub fn to_auction_error(program_err: TransactionError) -> AuctionContractError {
    match program_err {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => {
            FromPrimitive::from_u32(code).unwrap()
        }
        _ => unimplemented!(),
    }
}

#[allow(unused)]
pub struct TestContractConfig {
    pub auction_owner: TestUser,
    pub auction_id: [u8; 32],
}

pub struct TestUser {
    pub keypair: Keypair,
}

impl TestUser {
    pub async fn new(testbench: &mut Testbench) -> Self {
        let keypair = Keypair::new();

        // send lamports to user
        let instruction = system_instruction::transfer(
            &testbench.payer().pubkey(),
            &keypair.pubkey(),
            150_000_000,
        );

        let payer = testbench.clone_payer();

        testbench
            .process_transaction(&[instruction], &payer, None)
            .await
            .unwrap();

        Self { keypair }
    }
}

#[allow(unused)]
pub async fn warp_to_cycle_end(testbench: &mut Testbench, auction_id: [u8; 32]) {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await;
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(&auction_root_state_pubkey)
        .await;

    let current_time = testbench.block_time().await;
    let warp_duration = auction_cycle_state.end_time - current_time + 1;

    if warp_duration > 1 {
        testbench.warp_n_seconds(warp_duration).await;
    }

    let current_time = testbench.block_time().await;
    assert!(auction_cycle_state.end_time < current_time);
}

#[allow(unused)]
pub async fn get_next_child_edition(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> u64 {
    let nft_data = get_nft_data(testbench, auction_root_state_pubkey)
        .await
        .unwrap();

    let master_edition_data = testbench
        .get_and_deserialize_account_data::<MasterEditionV2>(&nft_data.master_edition)
        .await;
    master_edition_data.supply + 1
}

#[allow(unused)]
pub async fn get_nft_data(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> Option<NftData> {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await;
    match auction_root_state.token_config {
        TokenConfig::Token(_) => None,
        TokenConfig::Nft(nft_data) => Some(nft_data),
    }
}

#[allow(unused)]
pub async fn get_token_data(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> Option<TokenData> {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await;
    match auction_root_state.token_config {
        TokenConfig::Token(token_data) => Some(token_data),
        TokenConfig::Nft(_) => None,
    }
}

#[allow(unused)]
pub async fn get_current_cycle_number(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> u64 {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await;
    auction_root_state.status.current_auction_cycle
}

#[allow(unused)]
pub async fn get_top_bid(
    testbench: &mut Testbench,
    auction_cycle_state_pubkey: &Pubkey,
) -> Option<BidData> {
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(auction_cycle_state_pubkey)
        .await;
    auction_cycle_state
        .bid_history
        .get_last_element()
        .map(|elem| elem.clone())
}

#[allow(unused)]
pub async fn get_top_bidder_pubkey(
    testbench: &mut Testbench,
    auction_cycle_state_pubkey: &Pubkey,
) -> Option<Pubkey> {
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(auction_cycle_state_pubkey)
        .await;
    auction_cycle_state
        .bid_history
        .get_last_element()
        .map(|bid_data| bid_data.bidder_pubkey)
}

#[allow(unused)]
pub async fn get_state_pubkeys(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
) -> (Pubkey, Pubkey) {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let cycle_number = get_current_cycle_number(testbench, &auction_root_state_pubkey).await;
    let cycle_number_bytes = cycle_number.to_le_bytes();
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &get_auction_cycle_state_seeds(&auction_root_state_pubkey, &cycle_number_bytes),
        &CONTRACT_ID,
    );

    (auction_root_state_pubkey, auction_cycle_state_pubkey)
}

#[allow(unused)]
pub async fn close_cycle_transaction(
    testbench: &mut Testbench,
    payer_keypair: &Keypair,
    auction_id: [u8; 32],
    auction_owner_pubkey: &Pubkey,
    token_type: TokenType,
) -> Result<i64, AuctionContractError> {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await;

    let next_cycle_num = get_current_cycle_number(testbench, &auction_root_state_pubkey).await;

    let mut close_auction_cycle_args = CloseAuctionCycleArgs {
        payer_pubkey: payer_keypair.pubkey(),
        auction_owner_pubkey: *auction_owner_pubkey,
        top_bidder_pubkey: get_top_bidder_pubkey(testbench, &auction_cycle_state_pubkey).await,
        auction_id,
        next_cycle_num,
        token_type,
    };

    let close_auction_cycle_ix = close_auction_cycle(&close_auction_cycle_args);

    let payer_balance_before = testbench
        .get_account_lamports(&payer_keypair.pubkey())
        .await;
    let close_cycle_result = testbench
        .process_transaction(&[close_auction_cycle_ix], payer_keypair, None)
        .await;
    let payer_balance_after = testbench
        .get_account_lamports(&payer_keypair.pubkey())
        .await;

    if close_cycle_result.is_err() {
        return Err(to_auction_error(close_cycle_result.err().unwrap()));
    }

    Ok(payer_balance_after as i64 - payer_balance_before as i64)
}

#[allow(unused)]
pub async fn freeze_auction_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    auction_owner_keypair: &Keypair,
) -> Result<(), AuctionContractError> {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await;

    let freeze_args = FreezeAuctionArgs {
        auction_owner_pubkey: auction_owner_keypair.pubkey(),
        auction_id,
        top_bidder_pubkey: get_top_bidder_pubkey(testbench, &auction_cycle_state_pubkey).await,
        cycle_number: get_current_cycle_number(testbench, &auction_root_state_pubkey).await,
    };
    let freeze_instruction = freeze_auction(&freeze_args);
    let freeze_result = testbench
        .process_transaction(&[freeze_instruction], auction_owner_keypair, None)
        .await;

    if freeze_result.is_err() {
        return Err(to_auction_error(freeze_result.err().unwrap()));
    }

    Ok(())
}

#[allow(unused)]
pub async fn verify_auction_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    contract_admin_keypair: &Keypair,
) -> Result<i64, AuctionContractError> {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await;

    let verify_args = VerifyAuctionArgs {
        contract_admin_pubkey: contract_admin_keypair.pubkey(),
        auction_id,
    };
    let verify_instruction = verify_auction(&verify_args);

    let payer_balance_before = testbench
        .get_account_lamports(&contract_admin_keypair.pubkey())
        .await;
    let verify_result = testbench
        .process_transaction(&[verify_instruction], contract_admin_keypair, None)
        .await;
    let payer_balance_after = testbench
        .get_account_lamports(&contract_admin_keypair.pubkey())
        .await;

    if verify_result.is_err() {
        return Err(to_auction_error(verify_result.err().unwrap()));
    }

    Ok(payer_balance_after as i64 - payer_balance_before as i64)
}

#[allow(unused)]
pub async fn claim_funds_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    auction_owner: &Keypair,
    amount: u64,
) -> Result<i64, AuctionContractError> {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let payer = testbench.clone_payer();

    let claim_funds_args = ClaimFundsArgs {
        auction_owner_pubkey: auction_owner.pubkey(),
        auction_id,
        cycle_number: get_current_cycle_number(testbench, &auction_root_state_pubkey).await,
        amount,
    };

    let claim_funds_ix = claim_funds(&claim_funds_args);

    let payer_balance_before = testbench
        .get_account_lamports(&auction_owner.pubkey())
        .await;
    let claim_result = testbench
        .process_transaction(&[claim_funds_ix], auction_owner, None)
        .await;
    let payer_balance_after = testbench
        .get_account_lamports(&auction_owner.pubkey())
        .await;

    if claim_result.is_err() {
        return Err(to_auction_error(claim_result.err().unwrap()));
    }

    Ok(payer_balance_after as i64 - payer_balance_before as i64)
}

#[allow(unused)]
pub async fn delete_auction_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    auction_owner_pubkey: &Pubkey,
    contract_admin_keypair: &Keypair,
) -> Result<(), AuctionContractError> {
    let (auction_root_state_pubkey, _) =
        Pubkey::find_program_address(&get_auction_root_state_seeds(&auction_id), &CONTRACT_ID);

    let mut delete_auction_args = DeleteAuctionArgs {
        contract_admin_pubkey: contract_admin_keypair.pubkey(),
        auction_owner_pubkey: *auction_owner_pubkey,
        auction_id,
        current_auction_cycle: get_current_cycle_number(testbench, &auction_root_state_pubkey)
            .await,
        num_of_cycles_to_delete: RECOMMENDED_CYCLE_STATES_DELETED_PER_CALL,
    };

    let delete_auction_ix = delete_auction(&delete_auction_args);
    let delete_result = testbench
        .process_transaction(&[delete_auction_ix], contract_admin_keypair, None)
        .await;

    if delete_result.is_err() {
        return Err(to_auction_error(delete_result.err().unwrap()));
    }

    Ok(())
}

#[allow(unused)]
pub async fn place_bid_transaction(
    testbench: &mut Testbench,
    auction_id: [u8; 32],
    user_keypair: &Keypair,
    amount: u64,
) -> Result<i64, AuctionContractError> {
    let (auction_root_state_pubkey, auction_cycle_state_pubkey) =
        get_state_pubkeys(testbench, auction_id).await;

    let mut place_bid_args = PlaceBidArgs {
        user_main_pubkey: user_keypair.pubkey(),
        auction_id,
        cycle_number: get_current_cycle_number(testbench, &auction_root_state_pubkey).await,
        top_bidder_pubkey: get_top_bidder_pubkey(testbench, &auction_cycle_state_pubkey).await,
        amount,
    };
    let bid_instruction = place_bid(&place_bid_args);

    let payer_balance_before = testbench.get_account_lamports(&user_keypair.pubkey()).await;
    let bid_result = testbench
        .process_transaction(&[bid_instruction], user_keypair, None)
        .await;
    let payer_balance_after = testbench.get_account_lamports(&user_keypair.pubkey()).await;

    if bid_result.is_err() {
        return Err(to_auction_error(bid_result.err().unwrap()));
    }

    Ok(payer_balance_after as i64 - payer_balance_before as i64)
}

#[allow(unused)]
pub async fn initialize_new_auction_custom(
    testbench: &mut Testbench,
    auction_owner: &Keypair,
    auction_config: &AuctionConfig,
    auction_id: [u8; 32],
    create_token_args: CreateTokenArgs,
) -> Result<(), TransactionError> {
    let initialize_auction_args = InitializeAuctionArgs {
        auction_owner_pubkey: auction_owner.pubkey(),
        auction_id,
        auction_config: *auction_config,
        auction_name: auction_id,
        auction_description: AuctionDescription {
            description: MaxLenString::new("Cool description".to_string()),
            socials: vec![MaxLenString::new("https://www.gold.xyz".to_string())]
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
}

#[allow(unused)]
pub async fn initialize_new_auction(
    testbench: &mut Testbench,
    auction_owner: &Keypair,
    auction_config: &AuctionConfig,
    auction_id: [u8; 32],
    token_type: TokenType,
) -> Result<i64, AuctionContractError> {
    let initialize_auction_args = InitializeAuctionArgs::new_test(
        auction_owner.pubkey(),
        *auction_config,
        auction_id,
        token_type,
    );
    let instruction = initialize_auction(&initialize_auction_args);

    let payer_balance_before = testbench
        .get_account_lamports(&auction_owner.pubkey())
        .await;
    let initialize_auction_result = testbench
        .process_transaction(&[instruction], auction_owner, None)
        .await;
    let payer_balance_after = testbench
        .get_account_lamports(&auction_owner.pubkey())
        .await;

    if initialize_auction_result.is_err() {
        return Err(to_auction_error(initialize_auction_result.err().unwrap()));
    }

    Ok(payer_balance_after as i64 - payer_balance_before as i64)
}

#[allow(unused)]
pub async fn get_auction_cycle_pubkey(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> Pubkey {
    let auction_root_state = testbench
        .get_and_deserialize_account_data::<AuctionRootState>(auction_root_state_pubkey)
        .await;

    let cycle_number_bytes = auction_root_state
        .status
        .current_auction_cycle
        .to_le_bytes();
    let (auction_cycle_state_pubkey, _) = Pubkey::find_program_address(
        &get_auction_cycle_state_seeds(auction_root_state_pubkey, &cycle_number_bytes),
        &CONTRACT_ID,
    );

    auction_cycle_state_pubkey
}

#[allow(unused)]
pub async fn is_existing_account(testbench: &mut Testbench, account_pubkey: &Pubkey) -> bool {
    testbench
        .client()
        .get_account(*account_pubkey)
        .await
        .unwrap()
        .is_some()
}

#[allow(unused)]
pub async fn get_auction_cycle_state(
    testbench: &mut Testbench,
    auction_root_state_pubkey: &Pubkey,
) -> (Pubkey, AuctionCycleState) {
    let auction_cycle_state_pubkey =
        get_auction_cycle_pubkey(testbench, auction_root_state_pubkey).await;
    let auction_cycle_state = testbench
        .get_and_deserialize_account_data::<AuctionCycleState>(&auction_cycle_state_pubkey)
        .await;

    (auction_cycle_state_pubkey, auction_cycle_state)
}

#[allow(unused)]
pub async fn testbench_setup() -> (Testbench, TestUser) {
    let program_id = agsol_gold_contract::id();
    let testbench_program = TestbenchProgram {
        name: "agsol_gold_contract",
        id: program_id,
        process_instruction: processor!(agsol_gold_contract::processor::process),
    };

    // load metadata program binary
    let meta_program_id = metaplex_token_metadata::id();
    let meta_program = TestbenchProgram {
        name: "spl_token_metadata",
        id: meta_program_id,
        process_instruction: None,
    };

    let mut testbench = Testbench::new(&[testbench_program, meta_program]).await;
    let initialize_contract_args = InitializeContractArgs {
        contract_admin: testbench.payer().pubkey(),
        withdraw_authority: testbench.payer().pubkey(),
    };
    let init_contract_ix = initialize_contract(&initialize_contract_args);
    testbench
        .process_transaction(&[init_contract_ix], &testbench.clone_payer(), None)
        .await
        .unwrap();

    let auction_owner = TestUser::new(&mut testbench).await;

    (testbench, auction_owner)
}

#[allow(unused)]
pub async fn get_account_lamports(testbench: &mut Testbench, account_pubkey: &Pubkey) -> u64 {
    let account = testbench
        .client()
        .get_account(*account_pubkey)
        .await
        .unwrap()
        .unwrap();
    account.lamports
}
