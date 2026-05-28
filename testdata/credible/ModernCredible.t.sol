// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.28;

import "ds-test/test.sol";
import "cheats/Vm.sol";

enum AssertionSpec {
    Legacy,
    Reshiram,
    Experimental
}

interface PhEvm {
    struct ForkId {
        uint8 forkType;
        uint256 callIndex;
    }

    function loadStateAt(address target, bytes32 slot, ForkId calldata fork) external view returns (bytes32 value);
}

interface SpecRecorder {
    function registerAssertionSpec(AssertionSpec spec) external view;
}

interface TriggerRecorder {
    function registerCallTrigger(bytes4 fnSelector, bytes4 triggerSelector) external view;
}

abstract contract Assertion {
    PhEvm constant ph = PhEvm(0x4461812e00718ff8D80929E3bF595AEaaa7b881E);
    SpecRecorder constant specRecorder = SpecRecorder(address(uint160(uint256(keccak256("SpecRecorder")))));
    TriggerRecorder constant triggerRecorder = TriggerRecorder(address(uint160(uint256(keccak256("TriggerRecorder")))));

    function triggers() external view virtual;

    function registerAssertionSpec(AssertionSpec spec) internal {
        (bool ok,) =
            address(specRecorder).call(abi.encodeWithSelector(SpecRecorder.registerAssertionSpec.selector, spec));
        require(ok, "spec registration failed");
    }

    function registerCallTrigger(bytes4 fnSelector, bytes4 triggerSelector) internal view {
        triggerRecorder.registerCallTrigger(fnSelector, triggerSelector);
    }

    function _preTx() internal pure returns (PhEvm.ForkId memory) {
        return PhEvm.ForkId({forkType: 0, callIndex: 0});
    }

    function _postTx() internal pure returns (PhEvm.ForkId memory) {
        return PhEvm.ForkId({forkType: 1, callIndex: 0});
    }
}

contract ModernCounter {
    uint256 public value;

    function set(uint256 value_) external {
        value = value_;
    }

    function increment() external {
        value += 1;
    }
}

contract ModernCounterAssertion is Assertion {
    ModernCounter immutable counter;
    bytes32 constant VALUE_SLOT = bytes32(uint256(0));

    constructor(ModernCounter counter_) {
        registerAssertionSpec(AssertionSpec.Reshiram);
        counter = counter_;
    }

    function triggers() external view override {
        registerCallTrigger(this.assertValueIsOne.selector, ModernCounter.set.selector);
        registerCallTrigger(this.assertValueIsTwo.selector, ModernCounter.set.selector);
        registerCallTrigger(this.assertPrePostAndSingleApply.selector, ModernCounter.increment.selector);
    }

    function assertValueIsOne() external view {
        require(_postValue() == 1, "post value is not one");
    }

    function assertValueIsTwo() external view {
        require(_postValue() == 2, "post value is not two");
    }

    function assertPrePostAndSingleApply() external view {
        require(_preValue() == 0, "pre value is not zero");
        require(_postValue() == 1, "post value is not one");
    }

    function _preValue() internal view returns (uint256) {
        return uint256(ph.loadStateAt(address(counter), VALUE_SLOT, _preTx()));
    }

    function _postValue() internal view returns (uint256) {
        return uint256(ph.loadStateAt(address(counter), VALUE_SLOT, _postTx()));
    }
}

contract ModernCredibleTest is DSTest {
    Vm constant cl = Vm(HEVM_ADDRESS);

    ModernCounter counter;

    function setUp() public {
        counter = new ModernCounter();
    }

    function testRegisterCallTriggerAssertionPasses() public {
        cl.assertion(address(counter), _assertionCode(), ModernCounterAssertion.assertValueIsOne.selector);

        counter.set(1);

        assertEq(counter.value(), 1);
    }

    function testRegisterCallTriggerAssertionCanFailUnderExpectRevert() public {
        cl.assertion(address(counter), _assertionCode(), ModernCounterAssertion.assertValueIsTwo.selector);
        cl.expectRevert(bytes("post value is not two"));

        counter.set(1);

        assertEq(counter.value(), 0);
    }

    function testPrePostStateReadsTxDiffAndOuterExecutionAppliesOnce() public {
        cl.assertion(address(counter), _assertionCode(), ModernCounterAssertion.assertPrePostAndSingleApply.selector);

        counter.increment();

        assertEq(counter.value(), 1);
    }

    function _assertionCode() internal view returns (bytes memory) {
        return abi.encodePacked(type(ModernCounterAssertion).creationCode, abi.encode(counter));
    }
}
