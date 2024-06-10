use alloy_chains::Chain;
use alloy_provider::{Network, Provider};
use alloy_rpc_types::{Block, BlockNumberOrTag};
use alloy_transport::Transport;
use dashmap::DashMap;
use quick_cache::sync::Cache;
use revm::primitives::{Address, Bytecode};

#[allow(dead_code)]
struct AccountCodeCache {
    /// Account with  code and the earliest block it had code.
    pub with_code: Cache<(Address, Chain), (u64, Bytecode)>,
    /// Account with no code and the latest block it had no code.
    pub no_code: Cache<(Address, Chain), u64>,
}

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
#[derive(Clone, Debug, Default, PartialEq)]
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
    pub async fn get_latest_block_number<N: Network, T: Transport + Clone, P: Provider<T, N>>(
        &self,
        provider: &P,
        fork_url: &str,
    ) -> eyre::Result<u64> {
        match self.latest_block_map.get(fork_url) {
            Some(block_number) => Ok(*block_number),
            None => {
                let block_number = provider.get_block_number().await?;
                self.set_latest_block_number(fork_url, block_number);
                Ok(block_number)
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use foundry_common::provider::ProviderBuilder;
    use foundry_test_utils::rpc::next_http_rpc_endpoint as fork_url;

    const FAKE_FORK_URL: &str = "http://fake.com";

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_get_chain_id() {
        let fork_url = fork_url();
        let good_provider = ProviderBuilder::new(&fork_url).build().unwrap();

        let bad_provider = ProviderBuilder::new(&FAKE_FORK_URL).build().unwrap();

        let environment_cache = EnvironmentCache::default();

        // Fails with bad provider
        assert!(environment_cache.get_chain_id(&bad_provider, &fork_url).await.is_err());

        //Succeeds with good provider, caches the chain id
        assert_eq!(environment_cache.get_chain_id(&good_provider, &fork_url).await.unwrap(), 1);

        //Succeeds with bad provider, returns the cached chain id
        assert_eq!(environment_cache.get_chain_id(&bad_provider, &fork_url).await.unwrap(), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_get_block_env_by_number() {
        let fork_url = fork_url();
        let good_provider = ProviderBuilder::new(&fork_url).build().unwrap();

        let bad_provider = ProviderBuilder::new(&FAKE_FORK_URL).build().unwrap();

        let environment_cache = EnvironmentCache::default();

        // Fails with bad provider
        assert!(environment_cache
            .get_block_env_by_number(&bad_provider, &fork_url, 1_000_000)
            .await
            .is_err());

        //Succeeds with good provider, caches the block env
        let block_env = environment_cache
            .get_block_env_by_number(&good_provider, &fork_url, 1_000_000)
            .await
            .unwrap();

        //Succeeds with bad provider, returns the cached block env
        assert_eq!(
            environment_cache
                .get_block_env_by_number(&bad_provider, &fork_url, 1_000_000)
                .await
                .unwrap(),
            block_env
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_latest_block_number() {
        let cache = EnvironmentCache::default();
        let fork_url = fork_url();
        let block_number = 1_000_000;

        let provider = ProviderBuilder::new(&fork_url).build().unwrap();
        assert!(cache.get_latest_block_number(&provider, &fork_url).await.is_ok());

        cache.set_latest_block_number(&fork_url, block_number);
        assert_eq!(
            cache.get_latest_block_number(&provider, &fork_url).await.unwrap(),
            block_number
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_get_fork_info() {
        let fork_url = fork_url();
        let good_provider = ProviderBuilder::new(&fork_url).build().unwrap();

        let bad_provider = ProviderBuilder::new(&FAKE_FORK_URL).build().unwrap();

        let cache = EnvironmentCache::default();

        // Fails with bad provider
        assert!(cache.get_fork_info(&bad_provider, &fork_url, 1_000_000).await.is_err());

        // Succeeds with good provider, caches the chain id and block env
        let (chain_id, block_env_0) =
            cache.get_fork_info(&good_provider, &fork_url, 1_000_000).await.unwrap();
        assert_eq!(chain_id, 1);

        // Succeeds with bad provider, returns the cached chain id and block env
        let (chain_id, block_env_1) =
            cache.get_fork_info(&bad_provider, &fork_url, 1_000_000).await.unwrap();
        assert_eq!(chain_id, 1);
        assert_eq!(block_env_0, block_env_1);
    }
}
