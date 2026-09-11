// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.28;

import "ds-test/test.sol";
import "cheats/Vm.sol";

contract TraceCounter {
    uint256 public value = 41;

    function increment() external {
        value++;
    }
}

// All precompile calls deliberately use raw calldata, with no precompile interfaces in the ABI.
contract TraceAssertion {
    address constant PH = 0x4461812e00718ff8D80929E3bF595AEaaa7b881E;
    TraceCounter immutable counter;

    struct Snapshot {
        uint8 kind;
        uint256 callIndex;
    }

    struct Filter {
        uint8 callType;
        uint32 minDepth;
        uint32 maxDepth;
        bool topLevelOnly;
        bool successOnly;
    }

    constructor(TraceCounter counter_) {
        counter = counter_;
        (bool ok,) = address(uint160(uint256(keccak256("SpecRecorder"))))
            .call(abi.encodeWithSignature("registerAssertionSpec(uint8)", uint8(1)));
        require(ok, "spec registration failed");
    }

    function triggers() external view {
        (bool ok,) = address(uint160(uint256(keccak256("TriggerRecorder"))))
            .staticcall(
                abi.encodeWithSignature(
                    "registerCallTrigger(bytes4,bytes4)", this.check.selector, TraceCounter.increment.selector
                )
            );
        require(ok, "trigger registration failed");
    }

    function check() external view {
        bytes32 beforeValue = abi.decode(
            query(
                abi.encodeWithSignature(
                    "loadStateAt(address,bytes32,(uint8,uint256))", address(counter), bytes32(0), Snapshot(0, 0)
                )
            ),
            (bytes32)
        );
        bytes32 afterValue = abi.decode(
            query(abi.encodeWithSignature("loadStateAt(bytes32,(uint8,uint256))", bytes32(0), Snapshot(1, 0))),
            (bytes32)
        );
        require(uint256(beforeValue) == 41 && uint256(afterValue) == 42, "wrong snapshots");
        query(
            abi.encodeWithSignature(
                "staticcallAt(address,bytes,uint64,(uint8,uint256))",
                address(counter),
                abi.encodeWithSignature("value()"),
                uint64(100000),
                Snapshot(1, 0)
            )
        );
        query(
            abi.encodeWithSignature(
                "matchingCalls(address,bytes4,(uint8,uint32,uint32,bool,bool),uint256)",
                address(counter),
                TraceCounter.increment.selector,
                Filter(1, 1, 10, false, true),
                uint256(10)
            )
        );
        (bool ok,) = PH.staticcall(
            abi.encodeWithSignature(
                "loadStateAt(address,bytes32,(uint8,uint256))", address(counter), bytes32(0), Snapshot(255, 0)
            )
        );
        require(!ok, "invalid snapshot should revert");
    }

    function query(bytes memory input) internal view returns (bytes memory) {
        (bool ok, bytes memory output) = PH.staticcall(input);
        require(ok, "precompile failed");
        return output;
    }
}

contract PrecompileTracesTest is DSTest {
    Vm constant cl = Vm(HEVM_ADDRESS);
    TraceCounter counter;

    function setUp() public {
        counter = new TraceCounter();
    }

    function testPrecompileTraces() public {
        bytes memory code = abi.encodePacked(type(TraceAssertion).creationCode, abi.encode(counter));
        cl.assertion(address(counter), code, TraceAssertion.check.selector);
        counter.increment();
        assertEq(counter.value(), 42);
    }
}
