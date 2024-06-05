use crate::{
    backend::{BlockEnvironment, EnvironmentCache},
    utils::apply_chain_and_block_specific_env_changes,
};
use alloy_primitives::{Address, U256};
use alloy_provider::{Network, Provider};
use alloy_rpc_types::Block;
use alloy_transport::Transport;
use foundry_common::NON_ARCHIVE_NODE_WARNING;

use revm::primitives::{BlockEnv, CfgEnv, Env, TxEnv};

use std::sync::Arc;

pub struct EnvironmentArgs<P> {
    pub provider: Arc<P>,
    pub fork_url: String,
    pub env_cache: Arc<EnvironmentCache>,
    pub memory_limit: u64,
    pub gas_price: Option<u128>,
    pub override_chain_id: Option<u64>,
    pub pin_block: Option<u64>,
    pub origin: Address,
    pub disable_block_gas_limit: bool,
}

/// Initializes a REVM block environment based on a forked
/// ethereum provider.
// todo(onbjerg): these bounds needed cus of the bounds in `Provider`, can simplify?
pub async fn environment<N: Network, T: Transport + Clone, P: Provider<T, N>>(
    EnvironmentArgs {
        provider,
        fork_url,
        env_cache,
        memory_limit,
        gas_price,
        override_chain_id,
        pin_block,
        origin,
        disable_block_gas_limit,
    }: EnvironmentArgs<P>,
) -> eyre::Result<(Env, Block)> {
    let block_number = if let Some(pin_block) = pin_block {
        pin_block
    } else {
        env_cache.get_latest_block_number(&fork_url).expect("latest block for url not set")
    };

    let (rpc_chain_id, BlockEnvironment { gas_price: fork_gas_price, block }) =
        env_cache.get_fork_info(&provider, &fork_url, block_number).await?;

    let block = if let Some(block) = block {
        block
    } else if let Ok(latest_block) = provider.get_block_number().await {
        // If the `eth_getBlockByNumber` call succeeds, but returns null instead of
        // the block, and the block number is less than equal the latest block, then
        // the user is forking from a non-archive node with an older block number.
        if block_number <= latest_block {
            error!("{NON_ARCHIVE_NODE_WARNING}");
        }
        eyre::bail!(
            "Failed to get block for block number: {}\nlatest block number: {}",
            block_number,
            latest_block
        );
    } else {
        eyre::bail!("Failed to get block for block number: {}", block_number)
    };

    let mut cfg = CfgEnv::default();
    cfg.chain_id = override_chain_id.unwrap_or(rpc_chain_id);
    cfg.memory_limit = memory_limit;
    cfg.limit_contract_code_size = Some(usize::MAX);
    // EIP-3607 rejects transactions from senders with deployed code.
    // If EIP-3607 is enabled it can cause issues during fuzz/invariant tests if the caller
    // is a contract. So we disable the check by default.
    cfg.disable_eip3607 = true;
    cfg.disable_block_gas_limit = disable_block_gas_limit;

    let mut env = Env {
        cfg,
        block: BlockEnv {
            number: U256::from(block.header.number.expect("block number not found")),
            timestamp: U256::from(block.header.timestamp),
            coinbase: block.header.miner,
            difficulty: block.header.difficulty,
            prevrandao: Some(block.header.mix_hash.unwrap_or_default()),
            basefee: U256::from(block.header.base_fee_per_gas.unwrap_or_default()),
            gas_limit: U256::from(block.header.gas_limit),
            ..Default::default()
        },
        tx: TxEnv {
            caller: origin,
            gas_price: U256::from(gas_price.unwrap_or(fork_gas_price)),
            chain_id: Some(override_chain_id.unwrap_or(rpc_chain_id)),
            gas_limit: block.header.gas_limit as u64,
            ..Default::default()
        },
    };

    apply_chain_and_block_specific_env_changes(&mut env, &block);

    Ok((env, block))
}
