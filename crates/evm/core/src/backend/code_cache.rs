use alloy_chains::Chain;
use alloy_provider::{Network, Provider};
use alloy_transport::{Transport, TransportResult};
use quick_cache::sync::Cache;
use revm::primitives::{Address, Bytes};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct CodeDetected {
    block_number: u64,
    code: Bytes,
}

#[derive(Debug, Default, Clone)]
struct CodeHistory {
    code_first_detected_at: Option<CodeDetected>,
    eoa_last_detected_at: Option<u64>,
}

#[derive(Debug)]
pub struct CodeCache(Cache<(Address, Chain), CodeHistory>);

impl Default for CodeCache {
    fn default() -> Self {
        Self(Cache::new(10_000))
    }
}

impl CodeCache {
    /// Get the code of an account at a specific block, using the cache if possible.
    /// If the code is not in the cache, it will be fetched from the provider and cached.
    pub async fn get_code<N: Network, T: Transport + Clone, P: Provider<T, N>>(
        &self,
        provider: &P,
        address: Address,
        chain: Chain,
        block_number: u64,
    ) -> TransportResult<Bytes> {
        if let Some(code) = self.check_cache(address, chain, block_number) {
            return Ok(code);
        }

        let code = provider.get_code_at(address, block_number.into()).await?;

        self.cache_code(address, chain, block_number, code.clone());

        Ok(code)
    }

    /// Check the cache for the code of an account at a specific block.
    /// Returns the code if it is in the cache, otherwise None.
    ///
    /// If the account had code at the time of the block or earlier, it had code at the time of the
    /// block. If the account had no code at the time of the block or later, it had no code at
    /// the time of the block.
    fn check_cache(&self, address: Address, chain: Chain, block_number: u64) -> Option<Bytes> {
        if let Some(CodeHistory { code_first_detected_at, eoa_last_detected_at }) =
            self.0.get(&(address, chain))
        {
            if let Some(CodeDetected { block_number: code_first_detected_at, code }) =
                code_first_detected_at
            {
                if code_first_detected_at <= block_number {
                    return Some(code.clone());
                }
            }

            if let Some(eoa_last_detected_at) = eoa_last_detected_at {
                if eoa_last_detected_at >= block_number {
                    return Some(Bytes::new());
                }
            }
        }

        None
    }

    /// Cache the code of an account at a specific block.
    fn cache_code(&self, address: Address, chain: Chain, block_number: u64, code: Bytes) {
        let entry: CodeHistory = self
            .0
            .get_or_insert_with(&(address, chain), || Ok::<CodeHistory, ()>(CodeHistory::default()))
            .map(|mut history| {
                if code.is_empty() {
                    history.eoa_last_detected_at = Some(block_number);
                } else {
                    history.code_first_detected_at = Some(CodeDetected { block_number, code });
                }
                history
            })
            .unwrap();

        self.0.insert((address, chain), entry);
    }
}

#[tokio::test]
async fn test_check_code_cache() {
    let cache = CodeCache::default();
    let address = Address::from([1; 20]);
    let chain = Chain::mainnet();
    let block_number = 1000;

    // Cache empty
    assert_eq!(cache.check_cache(address, chain, block_number), None);

    let code = Bytes::from(vec![1, 2, 3]);

    // Cache with code
    cache.cache_code(address, chain, block_number, code.clone());
    assert_eq!(cache.check_cache(address, chain, block_number), Some(code.clone()));
    assert_eq!(cache.check_cache(address, chain, block_number + 1), Some(code));

    assert_eq!(cache.check_cache(address, chain, block_number - 1), None);

    let block_number = block_number - 10;

    // Cache with no code
    cache.cache_code(address, chain, block_number, Bytes::new());
    assert_eq!(cache.check_cache(address, chain, block_number), Some(Bytes::new()));
    assert_eq!(cache.check_cache(address, chain, block_number - 1), Some(Bytes::new()));

    assert_eq!(cache.check_cache(address, chain, block_number + 1), None);
}

#[tokio::test]
async fn test_cache_code() {
    let cache = CodeCache::default();
    let address = Address::from([1; 20]);
    let chain = Chain::mainnet();
    let block_number = 1000;

    let code = Bytes::from(vec![1, 2, 3]);

    cache.cache_code(address, chain, block_number, code.clone());
    assert!(cache.0.get(&(address, chain)).unwrap().eoa_last_detected_at.is_none());
    assert_eq!(
        cache.0.get(&(address, chain)).unwrap().code_first_detected_at,
        Some(CodeDetected { block_number, code })
    );

    let code = Bytes::new();
    let block_number = block_number - 10;

    cache.cache_code(address, chain, block_number, code.clone());
    assert_eq!(cache.0.get(&(address, chain)).unwrap().eoa_last_detected_at, Some(block_number));
}

#[tokio::test]
async fn test_get_code() {}
