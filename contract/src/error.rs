use agsol_common::SignerPdaError;
#[cfg(feature = "test-bpf")]
use num_derive::FromPrimitive;
use solana_program::program_error::ProgramError;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "test-bpf", derive(FromPrimitive))]
pub enum AuctionContractError {
    InvalidInstruction = 500,         // 1f4
    AuctionCycleEnded = 501,          // 1f5
    AuctionFrozen = 502,              // 1f6
    AuctionAlreadyInitialized = 503,  // 1f7
    ContractAlreadyInitialized = 504, // 1f8
    AuctionIsInProgress = 505,        // 1f9
    InvalidSeeds = 506,               // 1fa
    InvalidBidAmount = 507,           // 1fb
    AuctionOwnerMismatch = 508,       // 1fc
    InvalidStartTime = 509,           // 1fd
    TopBidderAccountMismatch = 510,   // 1fe
    MasterEditionMismatch = 511,      // 1ff
    ChildEditionNumberMismatch = 512, // 200
    NftAlreadyExists = 513,           // 201
    InvalidClaimAmount = 514,         // 202
    AuctionEnded = 515,               // 203
    AuctionIdNotUnique = 516,         // 204
    ContractAdminMismatch = 517,      // 205
    AuctionIsActive = 518,            // 206
    MetadataManipulationError = 519,  // 207
    InvalidProgramAddress = 520,      // 208
    InvalidAccountOwner = 521,        // 209
    ArithmeticError = 522,            // 20a
    WithdrawAuthorityMismatch = 523,  // 20b
    AuctionPoolFull = 524,            // 20c
    ShrinkingPoolIsNotAllowed = 525,  // 20d
    InvalidMinimumBidAmount = 526,    // 20e
    InvalidPerCycleAmount = 527,      // 20f
    InvalidCyclePeriod = 528,         // 210
    AuctionIdNotAscii = 529,          // 211
    TokenAuctionInconsistency = 530,  // 212
    StringTooLong = 531,              // 213
    InvalidEncorePeriod = 532,        // 214
}

impl From<AuctionContractError> for ProgramError {
    fn from(e: AuctionContractError) -> Self {
        ProgramError::Custom(e as u32)
    }
}

impl From<SignerPdaError> for AuctionContractError {
    fn from(_: SignerPdaError) -> Self {
        AuctionContractError::InvalidSeeds
    }
}
