#[cfg(feature = "client")]
pub mod factory;

use crate::state::{AuctionConfig, AuctionDescription, AuctionId, AuctionName, CreateTokenArgs};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::clock::UnixTimestamp;
use solana_program::pubkey::Pubkey;

// NOTE could hold a reference to description and metadata args
// to avoid cloning them, in the factory, but performance is not
// crucial in that part of the code.
#[allow(clippy::large_enum_variant)]
#[repr(C)]
#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub enum AuctionInstruction {
    InitializeContract {
        withdraw_authority: Pubkey,
        initial_auction_pool_len: u32,
    },
    InitializeAuction {
        id: AuctionId,
        auction_name: AuctionName,
        auction_config: AuctionConfig,
        description: AuctionDescription,
        create_token_args: CreateTokenArgs,
        auction_start_timestamp: Option<UnixTimestamp>,
    },
    Freeze {
        id: AuctionId,
    },
    FilterAuction {
        id: AuctionId,
        filter: bool,
    },
    CloseAuctionCycle {
        id: AuctionId,
    },
    Bid {
        id: AuctionId,
        amount: u64,
    },
    ClaimFunds {
        id: AuctionId,
        amount: u64,
    },
    VerifyAuction {
        id: AuctionId,
    },
    AdminWithdraw {
        amount: u64,
    },
    AdminWithdrawReassign {
        new_withdraw_authority: Pubkey,
    },
    ReallocatePool {
        new_max_auction_num: u32,
    },
}
