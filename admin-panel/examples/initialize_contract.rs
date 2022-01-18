use agsol_gold_admin_panel::{
    parse_keypair, request_airdrop, InitializeContractOpt, MIN_BALANCE, TEST_ADMIN_SECRET,
};

use agsol_gold_contract::instruction::factory::{initialize_contract, InitializeContractArgs};
use agsol_gold_contract::pda::auction_pool_seeds;
use agsol_gold_contract::ID as GOLD_ID;

use log::{error, info, warn};
use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signer::keypair::{read_keypair_file, Keypair};
use solana_sdk::signer::Signer;
use solana_sdk::transaction::Transaction;
use structopt::StructOpt;

use anyhow::anyhow;

pub fn main() {
    env_logger::init();
    let opt = InitializeContractOpt::from_args();

    let (connection_url, should_airdrop) = if opt.mainnet {
        ("https://api.mainnet-beta.solana.com".to_owned(), false)
    } else if opt.devnet {
        ("https://api.devnet.solana.com".to_owned(), true)
    } else if opt.localnet {
        ("http://localhost:8899".to_owned(), true)
    } else {
        ("https://api.testnet.solana.com".to_owned(), true)
    };

    let connection = RpcClient::new_with_commitment(connection_url, CommitmentConfig::confirmed());
    // unwraps below are fine because we are working with pre-tested consts
    // or panicking during initializiation is acceptable in this case
    let contract_admin_keypair = parse_keypair(opt.contract_admin_keypair, &TEST_ADMIN_SECRET);

    let withdraw_authority_pubkey = if let Some(keypair_path) = opt.withdraw_authority_keypair {
        read_keypair_file(keypair_path).unwrap().pubkey()
    } else {
        contract_admin_keypair.pubkey()
    };

    if let Err(e) = try_main(
        &connection,
        &contract_admin_keypair,
        should_airdrop,
        &withdraw_authority_pubkey,
    ) {
        error!("{}", e);
    }
}

fn try_main(
    connection: &RpcClient,
    contract_admin_keypair: &Keypair,
    should_airdrop: bool,
    withdraw_authority_pubkey: &Pubkey,
) -> Result<(), anyhow::Error> {
    // AIRDROP IF NECESSARY
    let admin_balance = connection.get_balance(&contract_admin_keypair.pubkey())?;
    if admin_balance < MIN_BALANCE {
        warn!(
            "admin balance ({}) is below threshold ({})",
            admin_balance, MIN_BALANCE
        );
        if should_airdrop {
            request_airdrop(connection, contract_admin_keypair)?;
        }
    }

    if let Err(err) = check_contract_state(connection) {
        error!("error while initializing contract: {}", err);
    }

    let initialize_contract_args = InitializeContractArgs {
        contract_admin: contract_admin_keypair.pubkey(),
        withdraw_authority: *withdraw_authority_pubkey,
        initial_auction_pool_len: 300,
    };

    let initialize_contract_ix = initialize_contract(&initialize_contract_args);

    let latest_blockhash = connection.get_latest_blockhash()?;

    let transaction = Transaction::new_signed_with_payer(
        &[initialize_contract_ix],
        Some(&contract_admin_keypair.pubkey()),
        &[contract_admin_keypair],
        latest_blockhash,
    );

    let signature = connection.send_and_confirm_transaction(&transaction)?;
    info!("Gold contract initialized    signature: {:?}", signature);

    Ok(())
}

fn check_contract_state(connection: &RpcClient) -> Result<(), anyhow::Error> {
    let (pool_pubkey, _) = Pubkey::find_program_address(&auction_pool_seeds(), &GOLD_ID);
    if connection.get_account_data(&pool_pubkey).is_ok() {
        return Err(anyhow!("auction pool already exists."));
    }

    Ok(())
}
