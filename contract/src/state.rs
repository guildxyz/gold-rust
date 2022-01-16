use crate::error::AuctionContractError;
use crate::{MAX_BID_HISTORY_LENGTH, MAX_DESCRIPTION_LEN, MAX_SOCIALS_LEN, MAX_SOCIALS_NUM};

use agsol_borsh_schema::BorshSchema;
use agsol_common::{AccountState, MaxLenString, MaxLenVec, MaxSerializedLen};
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::clock::UnixTimestamp;
use solana_program::pubkey::Pubkey;

use metaplex_token_metadata::instruction::CreateMetadataAccountArgs;

/// A unique identifier of an auction.
///
/// It is the "slugified" [`AuctionName`].
pub type AuctionId = [u8; 32];
/// The name of the auction that may be up to 32 characters long.
pub type AuctionName = [u8; 32];
/// Vector of the most recent bids submitted to a given auction.
pub type BidHistory = MaxLenVec<BidData, MAX_BID_HISTORY_LENGTH>;
/// A string containing the description of the auction.
pub type DescriptionString = MaxLenString<MAX_DESCRIPTION_LEN>;
/// A string containing a social url (Discord, Telegram, etc.) of the auction.
pub type SocialsString = MaxLenString<MAX_SOCIALS_LEN>;
/// A vector containing socials (Discord, Telegram, etc.) of the auction.
pub type SocialsVec = MaxLenVec<SocialsString, MAX_SOCIALS_NUM>;

/// Provides key information on a given auction.
#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone)]
pub struct AuctionDescription {
    /// Description of what the auction is about.
    #[alias(String)]
    pub description: DescriptionString,
    /// Social platform information (Discord, Telegram, etc.) of the auction.
    #[alias(Vec<String>)]
    pub socials: SocialsVec,
    /// The amount of capital the fundraiser aims to raise.
    pub goal_treasury_amount: Option<u64>,
}

/// The main configuration parameters of an auction.
#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone, Copy)]
pub struct AuctionConfig {
    /// Duration of an auction cycle (in seconds).
    pub cycle_period: UnixTimestamp,
    /// Duration of the last bid required to complete the auction (in seconds).
    pub encore_period: UnixTimestamp,
    /// Number of auction cycles taking place throughout the fundraiser.
    pub number_of_cycles: Option<u64>,
    /// Minimum bid amount accepted (in lamports).
    pub minimum_bid_amount: u64,
}

/// Current status of the auction.
#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone)]
pub struct AuctionStatus {
    /// The current auction cycle, regardless of the auction being frozen or
    /// active.
    pub current_auction_cycle: u64,
    /// The auction might be frozen.
    pub is_frozen: bool,
    /// The auction is active until all auction cycles have passed.
    pub is_active: bool,
}

/// Data of an incoming bid to the contract.
#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone)]
pub struct BidData {
    /// The public key of the bidder's account.
    pub bidder_pubkey: Pubkey,
    /// The bid amount placed by the bidder (in lamports).
    pub bid_amount: u64,
}

/// Information required to start either an NFT or a token-based fundraiser.
#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum CreateTokenArgs {
    /// Parameters required to create the metadata of a standard
    /// [`Metaplex`](https://www.metaplex.com/) NFT
    Nft {
        metadata_args: CreateMetadataAccountArgs,
        is_repeating: bool,
    },
    /// Parameters describing a token-based auction.
    ///
    /// The `per_cycle_amount` is the amount of tokens being auctioned off in
    /// each auction round.
    Token { decimals: u8, per_cycle_amount: u64 },
}

// TODO: this does not need to derive BorshSchema
// as it is only used in the contract tests and auction bot
#[derive(BorshSchema, BorshSerialize, BorshDeserialize, Debug, Clone)]
pub enum TokenType {
    Nft,
    Token,
}

#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone)]
pub struct NftData {
    /// Public key of the master edition NFT that serves as a base for the
    /// "child" NFTs minted in each auction cycle.
    pub master_edition: Pubkey,
    pub is_repeating: bool,
}

#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone)]
pub struct TokenData {
    /// Public key of the auctioned token's mint account.
    pub mint: Pubkey,
    /// Number of tokens auctioned off in each auction cycle.
    pub per_cycle_amount: u64,
}

#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, Debug, Clone)]
pub enum TokenConfig {
    Nft(NftData),
    Token(TokenData),
}

/// The main state of a fundraiser that holds data persistent between auction
/// cycles.
#[repr(C)]
#[derive(
    BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, AccountState, Debug, Clone,
)]
pub struct AuctionRootState {
    /// Name of the auction.
    #[alias([u8; 32])]
    pub auction_name: AuctionName,
    /// Owner of the auction (has freeze authority).
    pub auction_owner: Pubkey,
    /// Description of the auction.
    pub description: AuctionDescription,
    /// Configuration parameters of the auction.
    pub auction_config: AuctionConfig,
    /// Configuration parameters of the auctioned tokens/NFTs.
    pub token_config: TokenConfig,
    /// Status of the auction.
    pub status: AuctionStatus,
    /// All-time total funds raised in this auction.
    pub all_time_treasury: u64,
    /// Currently claimable funds from closed cycles.
    pub available_funds: u64,
    /// Start timestamp of the auction (in seconds)
    pub start_time: UnixTimestamp,
    /// The auction can be verified by the contract owners.
    pub is_verified: bool,
}

/// State respective to a given auction cycle.
#[repr(C)]
#[derive(
    BorshSchema, BorshDeserialize, BorshSerialize, MaxSerializedLen, AccountState, Debug, Clone,
)]
pub struct AuctionCycleState {
    /// When the auction cycle will end (in seconds).
    pub end_time: UnixTimestamp,
    /// The most recent bids of the current auction cycle.
    #[alias(Vec<BidData>)]
    pub bid_history: BidHistory,
}

/// Pool of auctions containing the [`AuctionId`] of each auction
#[repr(C)]
#[derive(BorshSchema, BorshDeserialize, BorshSerialize, AccountState, Debug, Clone)]
pub struct AuctionPool {
    pub max_len: u32,
    #[alias(Vec<[u8; 32]>)]
    pub pool: Vec<AuctionId>,
}

impl AuctionPool {
    pub fn max_serialized_len(n: usize) -> Option<usize> {
        let mul_result = AuctionId::MAX_SERIALIZED_LEN.checked_mul(n);
        if let Some(res) = mul_result {
            res.checked_add(4)
        } else {
            None
        }
    }

    pub fn new(max_len: u32) -> Self {
        Self {
            max_len,
            pool: Vec::new(),
        }
    }

    pub fn is_full(&self) -> bool {
        self.pool.len() == self.max_len as usize
    }

    pub fn try_insert_sorted(&mut self, auction_id: AuctionId) -> Result<(), AuctionContractError> {
        if self.is_full() {
            Err(AuctionContractError::AuctionPoolFull)
        } else {
            let search_result = self.pool.binary_search(&auction_id);
            match search_result {
                Ok(_) => Err(AuctionContractError::AuctionIdNotUnique),
                Err(index) => {
                    // not found in vec
                    self.pool.insert(index, auction_id);
                    Ok(())
                }
            }
        }
    }
    pub fn remove(&mut self, auction_id: &AuctionId) {
        let search_result = self.pool.binary_search(auction_id);
        if let Ok(index) = search_result {
            self.pool.remove(index);
        } // else there's nothing to remove
    }
}

#[repr(C)]
#[derive(BorshDeserialize, BorshSerialize, AccountState, MaxSerializedLen, Debug, Clone)]
pub struct ContractBankState {
    /// Address of the contract admin who may delete auctions.
    pub contract_admin: Pubkey,
    /// Address of the withdraw authority who may withdraw from the contract
    /// bank.
    pub withdraw_authority: Pubkey,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn max_serialized_len() {
        let auction_config = AuctionConfig {
            cycle_period: 86400,
            encore_period: 300,
            minimum_bid_amount: 10_000,
            number_of_cycles: Some(5),
        };

        let mut bid_history = BidHistory::new();
        let bid_data = BidData {
            bid_amount: 0,
            bidder_pubkey: Pubkey::new_unique(),
        };
        for _ in 0..10 {
            bid_history.cyclic_push(bid_data.clone());
        }
        let auction_owner = Pubkey::new_unique();

        let token_config = TokenConfig::Token(TokenData {
            per_cycle_amount: 20000,
            mint: Pubkey::new_unique(),
        });

        let auction_status = AuctionStatus {
            is_active: true,
            is_frozen: false,
            current_auction_cycle: 1,
        };

        let description_string: DescriptionString =
            DescriptionString::new("X".repeat(MAX_DESCRIPTION_LEN));

        assert_eq!(
            DescriptionString::MAX_SERIALIZED_LEN,
            description_string.try_to_vec().unwrap().len()
        );

        let long_link: SocialsString = "X".repeat(MAX_SOCIALS_LEN).try_into().unwrap();
        let socials_vec: SocialsVec = std::iter::repeat(long_link)
            .take(MAX_SOCIALS_NUM)
            .collect::<Vec<SocialsString>>()
            .try_into()
            .unwrap();

        assert_eq!(
            SocialsVec::MAX_SERIALIZED_LEN,
            socials_vec.try_to_vec().unwrap().len()
        );

        let auction_description = AuctionDescription {
            description: description_string,
            socials: socials_vec,
            goal_treasury_amount: Some(420_000_000_000),
        };

        assert_eq!(
            AuctionDescription::MAX_SERIALIZED_LEN,
            auction_description.try_to_vec().unwrap().len()
        );

        let auction_name = [1; 32];

        let root_state = AuctionRootState {
            auction_name,
            auction_owner,
            description: auction_description,
            auction_config,
            token_config,
            status: auction_status,
            all_time_treasury: 0,
            available_funds: 0,
            start_time: 0,
            is_verified: false,
        };

        assert_eq!(
            AuctionRootState::MAX_SERIALIZED_LEN,
            root_state.try_to_vec().unwrap().len()
        );

        let cycle_state = AuctionCycleState {
            end_time: 100_000_000,
            bid_history: bid_history.clone(),
        };

        assert_eq!(
            AuctionCycleState::MAX_SERIALIZED_LEN,
            cycle_state.try_to_vec().unwrap().len()
        );

        assert_eq!(AuctionPool::max_serialized_len(100), Some(3204));
        assert_eq!(AuctionPool::max_serialized_len(1000), Some(32004));
    }

    #[test]
    fn auction_pool_manipulation() {
        let mut auction_pool = AuctionPool::new(5);
        auction_pool.try_insert_sorted([4_u8; 32]).unwrap();
        auction_pool.try_insert_sorted([1_u8; 32]).unwrap();
        auction_pool.try_insert_sorted([2_u8; 32]).unwrap();
        assert_eq!(
            auction_pool.try_insert_sorted([1_u8; 32]),
            Err(AuctionContractError::AuctionIdNotUnique)
        );
        auction_pool.try_insert_sorted([3_u8; 32]).unwrap();
        auction_pool.try_insert_sorted([0_u8; 32]).unwrap();
        assert_eq!(
            auction_pool.try_insert_sorted([5_u8; 32]),
            Err(AuctionContractError::AuctionPoolFull)
        );
        assert_eq!(
            auction_pool.pool,
            vec![[0_u8; 32], [1_u8; 32], [2_u8; 32], [3_u8; 32], [4_u8; 32]]
        );
        auction_pool.remove(&[12_u8; 32]);
        assert_eq!(
            auction_pool.pool,
            vec![[0_u8; 32], [1_u8; 32], [2_u8; 32], [3_u8; 32], [4_u8; 32]]
        );
        auction_pool.remove(&[2_u8; 32]);
        auction_pool.remove(&[4_u8; 32]);
        assert_eq!(auction_pool.pool, vec![[0_u8; 32], [1_u8; 32], [3_u8; 32]]);
        // 4 + 4 + 3 * 32
        assert_eq!(auction_pool.try_to_vec().unwrap().len(), 104);
    }
}
