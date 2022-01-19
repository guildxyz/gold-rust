use std::path::PathBuf;
use structopt::StructOpt;

// default option is deploying on the testnet
#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct AdminWithdrawOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(
        long,
        short = "-withdraw-authority",
        help("The withdraw authority's keypair (default = test admin)")
    )]
    pub withdraw_authority_keypair: Option<PathBuf>,
    #[structopt(long, short = "-a", help("The amount to withdraw from contract bank"))]
    pub amount: u64,
}

#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct AuctionBotOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(long, help("The auction bot's keypair file"))]
    pub keypair: Option<PathBuf>,
}

#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct FilterAuctionOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(long, help("The contract admin's keypair file"))]
    pub keypair: Option<PathBuf>,
    #[structopt(long, short = "-id", help("The id of the auction to filter."))]
    pub auction_id: String,
}

#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct InitializeContractOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(
        long,
        short = "-contract-admin",
        help("The contract admin's keypair file (default = test admin)")
    )]
    pub contract_admin_keypair: Option<PathBuf>,
    #[structopt(
        long,
        short = "-withdraw-authority",
        help("The withdraw authority's keypair (default = contract_admin_keypair)"),
        requires("contract_admin_keypair")
    )]
    pub withdraw_authority_keypair: Option<PathBuf>,
}

#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct ReallocatePoolOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(
        long,
        short = "-contract-admin",
        help("The contract admin's keypair file (default = test admin)")
    )]
    pub contract_admin_keypair: Option<PathBuf>,
    #[structopt(
        long,
        short = "-s",
        help("The reallocated auction pool's size (must be greater than current size)")
    )]
    pub size: u32,
}

#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct ReassignWithdrawOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(
        long,
        short = "-withdraw-authority",
        help("The withdraw authority's keypair (default = test admin)")
    )]
    pub withdraw_authority_keypair: Option<PathBuf>,
    #[structopt(
        long,
        short = "-new-authority",
        help("The new withdraw authority's keypair")
    )]
    pub new_withdraw_authority_keypair: PathBuf,
}

#[allow(unused)]
#[derive(Debug, StructOpt)]
#[structopt(about = "Choose a Solana cluster to connect to (default = testnet)")]
pub struct VerifyAuctionOpt {
    #[structopt(
        long,
        short = "-l",
        help("Sets connection url to localhost"),
        conflicts_with("mainnet"),
        conflicts_with("devnet")
    )]
    pub localnet: bool,
    #[structopt(
        long,
        short = "-d",
        help("Sets connection url to devnet"),
        conflicts_with("mainnet"),
        conflicts_with("localnet")
    )]
    pub devnet: bool,
    #[structopt(
        long,
        short = "-m",
        help("Sets connection url to mainnet"),
        requires("keypair")
    )]
    pub mainnet: bool,
    #[structopt(
        long,
        short = "-contract-admin",
        help("The contract admin's keypair file (default = test admin)")
    )]
    pub contract_admin_keypair: Option<PathBuf>,
    #[structopt(long, short = "-id", help("The id of the auction to verify"))]
    pub auction_id: String,
}
