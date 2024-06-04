use crate::fork::CreateFork;
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

/// Enum to represent the different types of evm data accesses
#[derive(PartialEq, Eq, Hash, Debug, Clone)]
pub enum AccessType {
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
        create_fork.evm_opts.fork_block_number.map(|b| StateLookup::RollAt(b)).unwrap_or_default()
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
    use revm::{
        primitives::Env,
        DatabaseRef,
    };
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

    #[test]
    fn test_create_fork_latest() {
        let mut db = Backend::spawn(None);
        let create_fork = CreateFork {
            enable_caching: false,
            url: "https://eth.llamarpc.com".to_string(),
            env: Env::default(),
            evm_opts: EvmOpts::default(),
        };

        db.create_fork(create_fork).unwrap();

        assert_eq!(db.fork_access_metadata.len(), 1);
        assert_eq!(db.fork_access_metadata.values().next().unwrap().1, StateLookup::RollN(0));

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

        db.create_fork(create_fork).unwrap();

        assert_eq!(db.fork_access_metadata.len(), 1);
        assert_eq!(db.fork_access_metadata.values().next().unwrap().1, StateLookup::RollAt(1));
    }
    #[test]
    fn test_basic_ref() {
        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap();

        let db = get_forked_db(None);

        let _ = db.basic_ref(weth).unwrap();
 
        let expected_access = Access {
            access_type: AccessType::Basic(weth.clone()),
            chain: Chain::default(),
            state_lookup: StateLookup::RollN(0),
        };

        assert_eq!(
            db.drain_accesses_and_collect(),
            vec![expected_access]
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_load_state() {
        let weth = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".parse::<Address>().unwrap();

        let data_accesses = vec![
            Access {
                access_type: AccessType::Basic(weth),
                chain: Chain::default(),
                state_lookup: StateLookup::RollN(0),
            },
            Access {
                access_type: AccessType::Storage(weth, U256::ZERO),
                chain: Chain::default(),
                state_lookup: StateLookup::RollN(5),
            },
            Access {
                access_type: AccessType::Storage(weth, U256::ZERO),
                chain: Chain::default(),
                state_lookup: StateLookup::RollN(0),
            },
            Access {
                access_type: AccessType::Basic(weth),
                chain: Chain::default(),
                state_lookup: StateLookup::RollAt(10_000_000),
            },
        ];

        let db = get_forked_db(None);

        assert!(db.active_fork().is_some());

        let run = |label: &str| {
            println!("run {label}");
            let now = std::time::Instant::now();
            db.load_accesses(
                &data_accesses,
                Chain::default(),
                69,
                "https://eth.llamarpc.com".to_string(),
            )
            .unwrap();
            println!("{}: {:?}", label, now.elapsed());
        };

        run("a");
        run("b");
    }

}

