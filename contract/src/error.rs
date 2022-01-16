use agsol_common::SignerPdaError;
#[cfg(feature = "test-bpf")]
use num_derive::FromPrimitive;
use solana_program::program_error::ProgramError;

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "test-bpf", derive(FromPrimitive))]
pub enum AuctionContractError {
    InvalidInstruction = 500,
    AuctionCycleEnded = 501,
    AuctionFrozen = 502,
    AuctionAlreadyInitialized = 503,
    ContractAlreadyInitialized = 504,
    AuctionIsInProgress = 505,
    InvalidSeeds = 506,
    InvalidBidAmount = 507,
    AuctionOwnerMismatch = 508,
    InvalidStartTime = 509,
    TopBidderAccountMismatch = 510,
    MasterEditionMismatch = 511,
    ChildEditionNumberMismatch = 512,
    NftAlreadyExists = 513,
    InvalidClaimAmount = 514,
    AuctionEnded = 515,
    AuctionIdNotUnique = 516,
    ContractAdminMismatch = 517,
    AuctionIsActive = 518,
    MetadataManipulationError = 519,
    InvalidProgramAddress = 520,
    InvalidAccountOwner = 521,
    ArithmeticError = 522,
    WithdrawAuthorityMismatch = 523,
    AuctionPoolFull = 524,
    ShrinkingPoolIsNotAllowed = 525,
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
