use alloy_provider::{Network, Provider};
use alloy_rpc_types::{Block, BlockNumberOrTag};
use alloy_transport::Transport;
use quick_cache::sync::Cache;
use dashmap::DashMap;

#[derive(Debug)]
pub struct EnvironmentCache {
    /// A map of fork url -> chain id
    chain_ids_by_fork_url: DashMap<String, u64>,
    /// A map of fork url -> latest block number
    latest_block_map: DashMap<String, u64>,
    /// A map of url & block number -> block environment
    block_env_map: Cache<(String, u64), BlockEnvironment>,
}

impl Default for EnvironmentCache {
    fn default() -> Self {
        Self {
            chain_ids_by_fork_url: DashMap::new(),
            latest_block_map: DashMap::new(),
            block_env_map: Cache::new(500),
        }
    }
}

/// Cached Data for a block
#[derive(Clone, Debug, Default)]
pub struct BlockEnvironment {
    /// The [`Block`] object for a given block
    pub block: Option<Block>,
    /// The gas_price for the block
    pub gas_price: u128,
}

impl EnvironmentCache {
    /// Gets the chain id for the given fork url
    async fn get_chain_id<N: Network, T: Transport + Clone, P: Provider<T, N>>(
        &self,
        provider: &P,
        fork_url: &str,
    ) -> eyre::Result<u64> {
        if let Some(chain_id) = self.chain_ids_by_fork_url.get(fork_url) {
            return Ok(*chain_id);
        }
        let chain_id = provider.get_chain_id().await?;
        self.chain_ids_by_fork_url.insert(fork_url.to_string(), chain_id);
        Ok(chain_id)
    }

    /// Fetches the block environment for the given fork url and block number
    async fn get_block_env_by_number<N: Network, T: Transport + Clone, P: Provider<T, N>>(
        &self,
        provider: &P,
        fork_url: &str,
        block_number: u64,
    ) -> eyre::Result<BlockEnvironment> {
        if let Some(block_env) = self.block_env_map.get(&(fork_url.to_owned(), block_number)) {
            // If the block is none, try to fetch it from the provider and cache it
            if block_env.block.is_none() {
                let block = provider
                    .get_block_by_number(BlockNumberOrTag::Number(block_number), false)
                    .await?;

                let block_env = BlockEnvironment { block, gas_price: block_env.gas_price };
                self.block_env_map.insert((fork_url.to_owned(), block_number), block_env.clone());
                Ok(block_env)
            } else {
                Ok(block_env.clone())
            }
        } else {
            let (block, gas_price) = tokio::try_join!(
                provider.get_block_by_number(BlockNumberOrTag::Number(block_number), false),
                provider.get_gas_price()
            )?;

            let block_env = BlockEnvironment { block, gas_price };
            self.block_env_map.insert((fork_url.to_owned(), block_number), block_env.clone());
            Ok(block_env)
        }
    }

    /// Gets the latest block number for the given fork url
    pub fn get_latest_block_number(&self, fork_url: &str) -> Option<u64> {
        self.latest_block_map.get(fork_url).map(|block_number| *block_number)
    }

    /// Sets the latest block number for the given fork url
    pub fn set_latest_block_number(&self, fork_url: &str, block_number: u64) {
        self.latest_block_map.insert(fork_url.to_string(), block_number);
    }

    /// Fetches the chain id and block environment for the given fork url and block number
    pub async fn get_fork_info<N: Network, T: Transport + Clone, P: Provider<T, N>>(
        &self,
        provider: &P,
        fork_url: &str,
        block_number: u64,
    ) -> eyre::Result<(u64, BlockEnvironment)> {
        tokio::try_join!(
            self.get_chain_id(provider, fork_url),
            self.get_block_env_by_number(provider, fork_url, block_number)
        )
    }
}

