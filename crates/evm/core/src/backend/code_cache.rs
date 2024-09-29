use alloy_chains::Chain;
use alloy_provider::{Network, Provider};
use alloy_transport::{Transport, TransportResult};
use quick_cache::sync::Cache;
use revm::primitives::{Address, Bytes};

/// Type alias for a block number.
type BlockNumber = u64;

/// Struct for cacheing code history of an account for a chain.
/// This is used for returning the correct code for a given block number, under the assumption that
/// code is immutable.
#[derive(Debug, Default, Clone)]
struct CodeCacheEntry {
    /// The earliest block number at which code was detected by a get_code request, along with the
    /// code. None if there has not been a get_code request that returned code for this address
    /// on this chain.
    code_detected: Option<(BlockNumber, Bytes)>,
    /// The latest block number at which no code was detected by a get_code request.
    /// None if there has not been a get_code request that returned no code for this address on
    /// this chain.
    no_code_detected_block_number: Option<BlockNumber>,
}

/// Struct for cacheing code history of an account for a chain.
#[derive(Debug)]
pub struct CodeCache(Cache<(Address, Chain), CodeCacheEntry>);

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
        block_number: BlockNumber,
    ) -> TransportResult<Bytes> {
        if let Some(code) = self.check_cache(address, chain, block_number) {
            return Ok(code);
        }

        let code = provider.get_code_at(address).block_id(block_number.into()).await?;

        self.cache_code(address, chain, block_number, code.clone());

        Ok(code)
    }

    /// Check the cache for the code of an account at a specific block.
    /// Returns the code if it is in the cache, otherwise None.
    ///
    /// If the account had code at the time of the block or earlier, it had code at the time of the
    /// block. If the account had no code at the time of the block or later, it had no code at
    /// the time of the block.
    fn check_cache(
        &self,
        address: Address,
        chain: Chain,
        block_number: BlockNumber,
    ) -> Option<Bytes> {
        if let Some(CodeCacheEntry { code_detected, no_code_detected_block_number }) =
            self.0.get(&(address, chain))
        {
            if let Some((code_detected, code)) = code_detected {
                if code_detected <= block_number {
                    return Some(code);
                }
            }

            if let Some(no_code_detected_block_number) = no_code_detected_block_number {
                if no_code_detected_block_number >= block_number {
                    return Some(Bytes::new());
                }
            }
        }

        None
    }

    /// Cache the code of an account at a specific block.
    fn cache_code(&self, address: Address, chain: Chain, block_number: BlockNumber, code: Bytes) {
        let entry: CodeCacheEntry = self
            .0
            .get_or_insert_with(&(address, chain), || {
                Ok::<CodeCacheEntry, ()>(CodeCacheEntry::default())
            })
            .map(|mut history| {
                if code.is_empty() {
                    history.no_code_detected_block_number = Some(block_number);
                } else {
                    history.code_detected = Some((block_number, code));
                }
                history
            })
            .unwrap();

        self.0.insert((address, chain), entry);
    }
}

#[test]
fn test_check_code_cache() {
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

#[test]
fn test_cache_code() {
    let cache = CodeCache::default();
    let address = Address::from([1; 20]);
    let chain = Chain::mainnet();
    let block_number = 1000;

    let code = Bytes::from(vec![1, 2, 3]);

    cache.cache_code(address, chain, block_number, code.clone());
    assert!(cache.0.get(&(address, chain)).unwrap().no_code_detected_block_number.is_none());
    assert_eq!(cache.0.get(&(address, chain)).unwrap().code_detected, Some((block_number, code)));

    let code = Bytes::new();
    let block_number = block_number - 10;

    cache.cache_code(address, chain, block_number, code);
    assert_eq!(
        cache.0.get(&(address, chain)).unwrap().no_code_detected_block_number,
        Some(block_number)
    );
}
