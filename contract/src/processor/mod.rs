mod admin_withdraw;
mod bid;
mod claim_funds;
mod close_auction_cycle;
mod filter_auction;
mod freeze;
mod initialize_auction;
mod initialize_contract;
mod reallocate_pool;
mod verify_auction;

use crate::assertions::*;
use crate::error::AuctionContractError;
use crate::instruction::AuctionInstruction;
use crate::pda::factory::*;
use crate::pda::*;
use crate::state::*;
use crate::utils::initialize_create_metadata_args;

use metaplex_token_metadata::instruction as meta_instruction;
use metaplex_token_metadata::ID as META_ID;

use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::borsh::try_from_slice_unchecked;
use solana_program::clock::Clock;
use solana_program::entrypoint::ProgramResult;
use solana_program::msg;
use solana_program::program::{invoke, invoke_signed};
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use solana_program::system_instruction;
use solana_program::sysvar::Sysvar;
use spl_token::instruction as token_instruction;
use spl_token::ID as TOKEN_ID;

use agsol_common::{AccountState, MaxSerializedLen, SignerPda};

pub use close_auction_cycle::{increment_name, increment_uri};

pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction: AuctionInstruction = try_from_slice_unchecked(instruction_data)?;
    match instruction {
        AuctionInstruction::InitializeContract {
            withdraw_authority,
            initial_auction_pool_len,
        } => initialize_contract::initialize_contract(
            program_id,
            accounts,
            withdraw_authority,
            initial_auction_pool_len,
        ),
        AuctionInstruction::InitializeAuction {
            id,
            auction_name,
            description,
            auction_config,
            create_token_args,
            auction_start_timestamp,
        } => initialize_auction::initialize_auction(
            program_id,
            accounts,
            id,
            auction_name,
            description,
            auction_config,
            create_token_args,
            auction_start_timestamp,
        ),
        AuctionInstruction::Bid { amount, id } => {
            bid::process_bid(program_id, accounts, id, amount)
        }
        AuctionInstruction::CloseAuctionCycle { id } => {
            close_auction_cycle::close_auction_cycle(program_id, accounts, id)
        }
        AuctionInstruction::Freeze { id } => freeze::freeze_auction(program_id, accounts, id),
        AuctionInstruction::FilterAuction { id, filter } => {
            filter_auction::filter_auction(program_id, accounts, id, filter)
        }
        AuctionInstruction::ClaimFunds { id, amount } => {
            claim_funds::process_claim_funds(program_id, accounts, id, amount)
        }
        AuctionInstruction::VerifyAuction { id } => {
            verify_auction::process_verify_auction(program_id, accounts, id)
        }
        AuctionInstruction::AdminWithdraw { amount } => {
            admin_withdraw::process_admin_withdraw(program_id, accounts, amount)
        }
        AuctionInstruction::AdminWithdrawReassign {
            new_withdraw_authority,
        } => admin_withdraw::process_admin_withdraw_reassign(
            program_id,
            accounts,
            new_withdraw_authority,
        ),
        AuctionInstruction::ReallocatePool {
            new_max_auction_num,
        } => reallocate_pool::reallocate_pool(program_id, accounts, new_max_auction_num),
    }
}
