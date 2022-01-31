use std::path::PathBuf;
use structopt::StructOpt;

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
    #[structopt(long, help("Auction to focus on (optional)"))]
    pub auction_id: Option<String>,
}
