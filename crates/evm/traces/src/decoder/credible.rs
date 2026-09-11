//! Address-scoped assertion precompile metadata from the executor's generated bindings.

use crate::CallTrace;
use alloy_dyn_abi::{JsonAbiExt, eip712::Resolver};
use alloy_json_abi::Function;
use alloy_primitives::{Address, Selector, map::AddressHashMap};
use alloy_sol_types::{SolCall, SolType};
use assertion_executor::{
    constants::PRECOMPILE_ADDRESS,
    phevm::sol_abi::{ISpecRecorder, ITriggerRecorder, PhEvm},
    runtime::spec_recorder::SPEC_ADDRESS,
    triggers::recorder::TRIGGER_RECORDER,
};
use std::{collections::BTreeMap, sync::LazyLock};

struct Precompile {
    label: &'static str,
    functions: BTreeMap<Selector, Function>,
}

fn function<C: SolCall>(types: &Resolver) -> Function {
    // SolType names retain struct names; resolve the SDK's struct metadata into ABI tuples.
    let returns =
        types.resolve(C::ReturnTuple::SOL_NAME).expect("executor return types are registered");
    Function::parse(&format!("{} returns {}", C::SIGNATURE, returns.sol_type_name()))
        .expect("executor bindings provide valid ABI types")
}

static REGISTRY: LazyLock<AddressHashMap<Precompile>> = LazyLock::new(|| {
    let mut types = Resolver::default();
    types.ingest_sol_struct::<PhEvm::Log>();
    types.ingest_sol_struct::<PhEvm::LogQuery>();
    types.ingest_sol_struct::<PhEvm::CallInputs>();
    types.ingest_sol_struct::<PhEvm::CallFilter>();
    types.ingest_sol_struct::<PhEvm::TriggerCall>();
    types.ingest_sol_struct::<PhEvm::TxObject>();
    types.ingest_sol_struct::<PhEvm::StaticCallResult>();
    types.ingest_sol_struct::<PhEvm::Erc20TransferData>();
    types.ingest_sol_struct::<PhEvm::ForkId>();
    types.ingest_sol_struct::<PhEvm::AnomalyContext>();
    types.ingest_sol_struct::<PhEvm::TriggerContext>();
    types.ingest_sol_struct::<PhEvm::OutflowContext>();
    types.ingest_sol_struct::<PhEvm::InflowContext>();
    types.ingest_sol_struct::<PhEvm::FlowRateContext>();

    macro_rules! functions {
        ($($call:ty),* $(,)?) => {
            [$((Selector::from(<$call>::SELECTOR), function::<$call>(&types))),*].into()
        };
    }
    AddressHashMap::from_iter([
        (
            PRECOMPILE_ADDRESS,
            Precompile {
                label: "PhEvm",
                functions: functions![
                    PhEvm::forkPreTxCall,
                    PhEvm::forkPostTxCall,
                    PhEvm::forkPreCallCall,
                    PhEvm::forkPostCallCall,
                    PhEvm::loadCall,
                    PhEvm::getLogsCall,
                    PhEvm::getAllCallInputsCall,
                    PhEvm::getCallInputsCall,
                    PhEvm::getStaticCallInputsCall,
                    PhEvm::getDelegateCallInputsCall,
                    PhEvm::getCallCodeInputsCall,
                    PhEvm::matchingCallsCall,
                    PhEvm::callinputAtCall,
                    PhEvm::callOutputAtCall,
                    PhEvm::getStateChangesCall,
                    PhEvm::forbidChangeForSlotCall,
                    PhEvm::forbidChangeForSlotsCall,
                    PhEvm::getAssertionAdopterCall,
                    PhEvm::getTxObjectCall,
                    PhEvm::loadStateAt_0Call,
                    PhEvm::loadStateAt_1Call,
                    PhEvm::staticcallAtCall,
                    PhEvm::conserveBalanceCall,
                    PhEvm::getLogsQueryCall,
                    PhEvm::getLogsForCallCall,
                    PhEvm::getErc20TransfersCall,
                    PhEvm::getErc20TransfersForTokensCall,
                    PhEvm::changedErc20BalanceDeltasCall,
                    PhEvm::reduceErc20BalanceDeltasCall,
                    PhEvm::changedMappingKeysCall,
                    PhEvm::mappingValueDiffCall,
                    PhEvm::contextCall,
                    PhEvm::outflowContextCall,
                    PhEvm::inflowContextCall,
                    PhEvm::anomalyContextCall,
                    PhEvm::outflowRateCall,
                    PhEvm::inflowRateCall,
                    PhEvm::mulDivDownCall,
                    PhEvm::mulDivUpCall,
                    PhEvm::normalizeDecimalsCall,
                    PhEvm::ratioGeCall,
                    PhEvm::oracleSanityCall,
                    PhEvm::oracleSanityAtCall,
                    PhEvm::assetsMatchSharePriceCall,
                    PhEvm::assetsMatchSharePriceAtCall,
                ],
            },
        ),
        (
            SPEC_ADDRESS,
            Precompile {
                label: "SpecRecorder",
                functions: functions![ISpecRecorder::registerAssertionSpecCall,],
            },
        ),
        (
            TRIGGER_RECORDER,
            Precompile {
                label: "TriggerRecorder",
                functions: functions![
                    ITriggerRecorder::registerCallTrigger_0Call,
                    ITriggerRecorder::registerCallTrigger_1Call,
                    ITriggerRecorder::registerStorageChangeTrigger_0Call,
                    ITriggerRecorder::registerStorageChangeTrigger_1Call,
                    ITriggerRecorder::registerBalanceChangeTriggerCall,
                    ITriggerRecorder::registerFnCallTriggerCall,
                    ITriggerRecorder::registerTxEndTriggerCall,
                    ITriggerRecorder::registerErc20ChangeTriggerCall,
                    ITriggerRecorder::watchCumulativeOutflowCall,
                    ITriggerRecorder::watchCumulativeInflowCall,
                    ITriggerRecorder::watchAnomalyCall,
                ],
            },
        ),
    ])
});

pub(super) fn label(address: Address) -> Option<&'static str> {
    REGISTRY.get(&address).map(|precompile| precompile.label)
}

/// Only recognize calls with valid inputs; malformed calls keep their raw presentation.
pub(crate) fn decoded_function(trace: &CallTrace) -> Option<&'static Function> {
    if trace.kind.is_any_create() {
        return None;
    }
    let selector = Selector::try_from(trace.data.get(..4)?).ok()?;
    let function = REGISTRY.get(&trace.address)?.functions.get(&selector)?;
    function.abi_decode_input(&trace.data[4..]).ok()?;
    Some(function)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CallKind, CallTraceArena, SparsedTraceArena, decoder::CallTraceDecoder, render_trace_arena,
    };
    use alloy_primitives::{B256, Bytes, U256, address};
    use alloy_sol_types::{SolError, SolValue};
    use revm::interpreter::InstructionResult;
    use std::path::Path;

    const TARGET: Address = address!("1111111111111111111111111111111111111111");

    fn load(target: bool, snapshot: u8) -> Bytes {
        let fork = PhEvm::ForkId { forkType: snapshot, callIndex: U256::ZERO };
        if target {
            PhEvm::loadStateAt_1Call { target: TARGET, slot: B256::ZERO, fork }.abi_encode().into()
        } else {
            PhEvm::loadStateAt_0Call { slot: B256::ZERO, fork }.abi_encode().into()
        }
    }

    fn trace(data: Bytes, output: Bytes) -> CallTrace {
        CallTrace {
            address: PRECOMPILE_ADDRESS,
            kind: CallKind::StaticCall,
            data,
            output,
            success: true,
            status: Some(InstructionResult::Return),
            ..Default::default()
        }
    }

    async fn render(decoder: &CallTraceDecoder, mut trace: CallTrace) -> String {
        trace.decoded = Some(Box::new(decoder.decode_function(&trace).await));
        let mut arena = CallTraceArena::default();
        arena.nodes_mut()[0].trace = trace;
        let arena = SparsedTraceArena {
            arena,
            ignored: Default::default(),
            diagnostics: Default::default(),
        };
        let before = serde_json::to_value(&arena).unwrap();
        let output = render_trace_arena(&arena);
        assert_eq!(arena.arena.nodes()[0].trace.kind, CallKind::StaticCall);
        assert_eq!(serde_json::to_value(&arena).unwrap(), before);
        assert_eq!(before["arena"][0]["trace"]["kind"], "STATICCALL");
        output
    }

    #[test]
    fn credible_registry_covers_sdk_selectors() {
        for (address, selectors) in [
            (PRECOMPILE_ADDRESS, PhEvm::PhEvmCalls::SELECTORS),
            (SPEC_ADDRESS, ISpecRecorder::ISpecRecorderCalls::SELECTORS),
            (TRIGGER_RECORDER, ITriggerRecorder::ITriggerRecorderCalls::SELECTORS),
        ] {
            let functions = &REGISTRY[&address].functions;
            assert_eq!(functions.len(), selectors.len());
            for selector in selectors {
                let selector = Selector::from(*selector);
                assert_eq!(functions[&selector].selector(), selector);
            }
        }
    }

    #[tokio::test]
    async fn credible_operations_and_returns() {
        let decoder = CallTraceDecoder::new().clone();
        let mut output = String::new();
        for (target, snapshot, value) in [(true, 0, 41u64), (true, 1, 42), (false, 1, 42)] {
            output.push_str(
                &render(
                    &decoder,
                    trace(
                        load(target, snapshot),
                        B256::from(U256::from(value)).abi_encode().into(),
                    ),
                )
                .await,
            );
        }
        let data = PhEvm::staticcallAtCall {
            target: TARGET,
            data: Bytes::from_static(&[0x12, 0x34, 0x56, 0x78]),
            gas_limit: 100,
            fork: PhEvm::ForkId { forkType: 1, callIndex: U256::ZERO },
        }
        .abi_encode()
        .into();
        output.push_str(
            &render(
                &decoder,
                trace(data, (true, Bytes::from_static(&[0xab, 0xcd])).abi_encode().into()),
            )
            .await,
        );
        let data = PhEvm::matchingCallsCall {
            target: TARGET,
            selector: [0x12, 0x34, 0x56, 0x78].into(),
            filter: PhEvm::CallFilter {
                callType: 1,
                minDepth: 1,
                maxDepth: 2,
                topLevelOnly: false,
                successOnly: true,
            },
            limit: U256::from(10),
        }
        .abi_encode()
        .into();
        let calls = vec![PhEvm::TriggerCall {
            callId: U256::from(7),
            parentCallId: U256::ZERO,
            caller: TARGET,
            target: TARGET,
            codeAddress: TARGET,
            selector: [0x12, 0x34, 0x56, 0x78].into(),
            depth: 1,
            callType: 1,
            success: true,
            value: U256::ZERO,
            input: Bytes::from_static(&[0xab, 0xcd]),
        }];
        output.push_str(&render(&decoder, trace(data, calls.abi_encode().into())).await);
        let mut reverted = trace(
            load(true, 2),
            alloy_sol_types::Revert::from("snapshot unavailable").abi_encode().into(),
        );
        reverted.success = false;
        reverted.status = Some(InstructionResult::Revert);
        output.push_str(&render(&decoder, reverted).await);
        for (address, data) in [
            (
                SPEC_ADDRESS,
                ISpecRecorder::registerAssertionSpecCall {
                    spec: assertion_executor::phevm::sol_abi::AssertionSpec::Reshiram,
                }
                .abi_encode(),
            ),
            (
                TRIGGER_RECORDER,
                ITriggerRecorder::registerCallTrigger_0Call { fnSelector: [1, 2, 3, 4].into() }
                    .abi_encode(),
            ),
            (
                TRIGGER_RECORDER,
                ITriggerRecorder::registerCallTrigger_1Call {
                    fnSelector: [1, 2, 3, 4].into(),
                    triggerSelector: [5, 6, 7, 8].into(),
                }
                .abi_encode(),
            ),
        ] {
            let mut call = trace(data.into(), Bytes::new());
            call.address = address;
            output.push_str(&render(&decoder, call).await);
        }
        snapbox::assert_data_eq!(
            output,
            snapbox::Data::read_from(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decoder/credible-operations.txt"),
                None
            )
        );
    }

    #[tokio::test]
    async fn credible_reset_labels_and_fallbacks() {
        let mut decoder = CallTraceDecoder::new().clone();
        let valid = trace(load(true, 0), B256::ZERO.abi_encode().into());
        let initial = render(&decoder, valid.clone()).await;
        decoder.clear_addresses();
        assert_eq!(render(&decoder, valid.clone()).await, initial);
        decoder.labels.insert(PRECOMPILE_ADDRESS, "CustomPhEvm".into());
        assert_eq!(decoder.decode_function(&valid).await.label.as_deref(), Some("CustomPhEvm"));
        decoder.disable_labels = true;
        assert_eq!(decoder.decode_function(&valid).await.label, None);
        decoder = CallTraceDecoder::new().clone();
        let mut output = String::new();
        for data in [
            Bytes::new(),
            Bytes::from_static(&[1, 2, 3]),
            Bytes::from_static(&[0xde, 0xad, 0xbe, 0xef]),
            valid.data.slice(..4),
        ] {
            let malformed = trace(data, Bytes::new());
            assert!(decoded_function(&malformed).is_none());
            assert!(decoder.decode_function(&malformed).await.call_data.is_none());
            output.push_str(&render(&decoder, malformed).await);
        }
        let malformed_output = trace(valid.data.clone(), Bytes::from_static(&[0xab]));
        output.push_str(&render(&decoder, malformed_output).await);
        let mut creation = valid.clone();
        creation.kind = CallKind::Create;
        assert!(decoded_function(&creation).is_none());
        let mut unrelated = valid.clone();
        unrelated.address = TARGET;
        assert!(decoded_function(&unrelated).is_none());
        output.push_str(&render(&decoder, unrelated).await);
        let ordinary = Function::parse("value() returns (uint256)").unwrap();
        decoder.functions.insert(ordinary.selector(), vec![ordinary.clone()]);
        let mut ordinary_trace =
            trace(ordinary.selector().to_vec().into(), U256::from(42).abi_encode().into());
        ordinary_trace.address = TARGET;
        output.push_str(&render(&decoder, ordinary_trace).await);
        snapbox::assert_data_eq!(
            output,
            snapbox::Data::read_from(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/decoder/credible-fallbacks.txt"),
                None
            )
        );
    }
}
