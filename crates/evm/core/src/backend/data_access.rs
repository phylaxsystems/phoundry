use crate::{
    backend::{DatabaseError, DatabaseRef},
    fork::{CreateFork, SharedBackend},
};
use alloy_chains::Chain;
use alloy_primitives::{Address, B256, U256};

/// Struct to represent an evm data access
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub struct Access {
    /// The data access type
    pub access_type: AccessType,
    /// The chain the data access is for
    pub chain: Chain,
    /// The point in state to look up the data access
    pub state_lookup: StateLookup,
}

impl RevmDbAccess {
    /// Executes the RevmDbAccess against the SharedBackend
    pub fn execute(&self, db: &mut SharedBackend) -> Result<(), DatabaseError> {
        match self {
            Self::Basic(addr) => {
                db.basic_ref(*addr)?;
            }
            Self::Storage(addr, key) => {
                db.storage_ref(*addr, *key)?;
            }
            Self::CodeByHash(hash) => {
                db.code_by_hash_ref(*hash)?;
            }
            Self::BlockHash(block_num) => {
                db.block_hash_ref(*block_num)?;
            }
        }
        Ok(())
    }
    /// Converts the RevmDbAccess to an Access
    pub fn to_access(self, chain: Chain, state_lookup: StateLookup) -> Access {
        Access { access_type: AccessType::RevmDbAccess(self), chain, state_lookup }
    }
}

/// Enum to represent the different types of evm data accesses
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum AccessType {
    /// Access to a block hash by the block number
    RevmDbAccess(RevmDbAccess),
    /// Create a fork with the given url
    CreateFork(String),
}

/// Enum to represent the different types of evm data accesses
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum RevmDbAccess {
    /// Access to a storage slot
    Storage(Address, U256),
    /// Access to a basic account
    Basic(Address),
    /// Access to a code hash
    CodeByHash(B256),
    /// Access to a block hash by the block number
    BlockHash(U256),
}

/// Enum to represent the different ways to look up state
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum StateLookup {
    RollN(i64),
    RollAt(u64),
    //RollTransaction(B256),
}

impl Default for StateLookup {
    fn default() -> Self {
        Self::RollN(0) //default to latest block
    }
}

impl From<&CreateFork> for StateLookup {
    fn from(create_fork: &CreateFork) -> Self {
        create_fork.evm_opts.fork_block_number.map(StateLookup::RollAt).unwrap_or_default()
    }
}

#[test]
fn test_default_state_lookup() {
    assert_eq!(StateLookup::default(), StateLookup::RollN(0));
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        backend::{Backend, DatabaseExt},
        fork::CreateFork,
        opts::EvmOpts,
    };
    use revm::{primitives::Env, DatabaseRef};
    const ENDPOINT: &str = "https://eth.llamarpc.com";

    fn get_forked_db(url: Option<String>) -> Backend {
        let create_fork = CreateFork {
            enable_caching: false,
            url: url.unwrap_or(ENDPOINT.to_string()),
            env: Env::default(),
            evm_opts: EvmOpts::default(),
        };
        Backend::spawn(Some(create_fork))
    }

    #[tokio::test]
    async fn test_create_fork_latest() {
        let db = get_forked_db(None);

        db.data_accesses.contains(&Access {
            access_type: AccessType::CreateFork(ENDPOINT.to_string()),
            chain: Chain::default(),
            state_lookup: StateLookup::RollN(0),
        });
    }

    #[test]
    fn test_create_fork_at_block() {
        let mut db = Backend::spawn(None);
        let create_fork = CreateFork {
            enable_caching: false,
            url: ENDPOINT.to_string(),
            env: Env::default(),
            evm_opts: EvmOpts { fork_block_number: Some(1), ..Default::default() },
        };

        db.create_fork(create_fork, StateLookup::RollAt(1)).unwrap();

        db.data_accesses.contains(&Access {
            access_type: AccessType::CreateFork(ENDPOINT.to_string()),
            chain: Chain::default(),
            state_lookup: StateLookup::RollAt(1),
        });
    }
    #[test]
    fn test_basic_ref() {
        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap();

        let db = get_forked_db(None);

        let _ = db.basic_ref(weth).unwrap();

        let expected_access = Access {
            access_type: AccessType::RevmDbAccess(RevmDbAccess::Basic(weth)),
            chain: Chain::default(),
            state_lookup: StateLookup::RollN(0),
        };

        assert_eq!(db.get_accesses(), vec![expected_access]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_load_state() {
        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap();

        let data_accesses = vec![
            Access {
                access_type: AccessType::RevmDbAccess(RevmDbAccess::Basic(weth)),
                chain: Chain::default(),
                state_lookup: StateLookup::RollN(0),
            },
            Access {
                access_type: AccessType::RevmDbAccess(RevmDbAccess::Storage(weth, U256::ZERO)),
                chain: Chain::default(),
                state_lookup: StateLookup::RollN(5),
            },
            Access {
                access_type: AccessType::RevmDbAccess(RevmDbAccess::Storage(weth, U256::ZERO)),
                chain: Chain::default(),
                state_lookup: StateLookup::RollN(0),
            },
            Access {
                access_type: AccessType::RevmDbAccess(RevmDbAccess::Basic(weth)),
                chain: Chain::default(),
                state_lookup: StateLookup::RollAt(10_000_000),
            },
        ];

        let db = get_forked_db(None);

        assert!(db.active_fork().is_some());

        let run = |label: &str| {
            println!("run {label}");
            let now = std::time::Instant::now();
            db.load_accesses(&data_accesses, Chain::default(), 69, ENDPOINT.to_string()).unwrap();
            println!("{}: {:?}", label, now.elapsed());
        };

        run("a");
        run("b");
    }
}
