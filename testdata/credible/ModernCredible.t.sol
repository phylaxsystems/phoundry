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
    function watchAnomaly(address target, bytes4 fnSelector, uint8 sensitivity) external view;
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

    function watchAnomaly(address target, bytes4 fnSelector, uint8 sensitivity) internal view {
        triggerRecorder.watchAnomaly(target, fnSelector, sensitivity);
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

// Registered at level 7, so a verdict of 7 or stricter fires it and a looser one does not.
uint8 constant WATCHED_LEVEL = 7;

/// An assertion behind an anomaly trigger. The body always reverts, so what a test observes is
/// purely *whether the trigger fired*: a revert with this message means it did, and forge's
/// "0 were executed" means it did not.
contract ModernAnomalyAssertion is Assertion {
    ModernCounter immutable counter;

    constructor(ModernCounter counter_) {
        registerAssertionSpec(AssertionSpec.Reshiram);
        counter = counter_;
    }

    function triggers() external view override {
        watchAnomaly(address(counter), this.assertNotAnomalous.selector, WATCHED_LEVEL);
    }

    function assertNotAnomalous() external pure {
        revert("anomaly trigger fired");
    }
}

contract ModernCredibleTest is DSTest {
    Vm constant cl = Vm(HEVM_ADDRESS);

    /// What forge reports when a staged assertion's trigger never fires.
    bytes constant NOT_EXECUTED = bytes("Expected 1 assertion to be executed, but 0 were executed.");

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

    function testMissingCallTriggerCanFailUnderExpectRevert() public {
        cl.assertion(address(counter), _assertionCode(), bytes4(keccak256("unregisteredAssertion()")));
        cl.expectRevert(bytes("Expected 1 assertion to be executed, but 0 were executed."));

        counter.set(1);

        assertEq(counter.value(), 0);
    }

    function testPrePostStateReadsTxDiffAndOuterExecutionAppliesOnce() public {
        cl.assertion(address(counter), _assertionCode(), ModernCounterAssertion.assertPrePostAndSingleApply.selector);

        counter.increment();

        assertEq(counter.value(), 1);
    }

    // ---- anomaly trigger: `cl.setAnomalyLevel` ----

    /// A verdict at least as strict as the registered level fires the trigger. This is the one
    /// test that proves the staged level reaches the subsystem at all, through the generated ABI.
    function testStagedVerdictAtOrAboveTheLevelFires() public {
        cl.setAnomalyLevel(address(counter), WATCHED_LEVEL);
        cl.assertion(address(counter), _anomalyCode(), ModernAnomalyAssertion.assertNotAnomalous.selector);
        cl.expectRevert(bytes("anomaly trigger fired"));

        counter.set(1);
    }

    /// A trigger registered at `L` fires iff `firesAt != 0 && L >= firesAt`, so a verdict that
    /// clears only a looser rung leaves it alone.
    function testLooserVerdictDoesNotFire() public {
        cl.setAnomalyLevel(address(counter), WATCHED_LEVEL + 1);
        cl.assertion(address(counter), _anomalyCode(), ModernAnomalyAssertion.assertNotAnomalous.selector);
        cl.expectRevert(NOT_EXECUTED);

        counter.set(1);
    }

    /// `0` is the "cleared nothing" sentinel. Staging it must leave the trigger inert rather than
    /// read as the strictest level.
    function testLevelZeroClearsNothing() public {
        cl.setAnomalyLevel(address(counter), 0);
        cl.assertion(address(counter), _anomalyCode(), ModernAnomalyAssertion.assertNotAnomalous.selector);
        cl.expectRevert(NOT_EXECUTED);

        counter.set(1);
    }

    /// A target nobody staged is not scored. That is the fail-open default.
    function testUnstagedTargetIsInert() public {
        cl.assertion(address(counter), _anomalyCode(), ModernAnomalyAssertion.assertNotAnomalous.selector);
        cl.expectRevert(NOT_EXECUTED);

        counter.set(1);
    }

    /// The staged map is consumed by the assertion that reads it. Without this, one staged verdict
    /// would silently arm every later assertion in the test and they would all pass for free.
    function testStagedVerdictIsConsumedByOneAssertion() public {
        cl.setAnomalyLevel(address(counter), WATCHED_LEVEL);
        cl.assertion(address(counter), _anomalyCode(), ModernAnomalyAssertion.assertNotAnomalous.selector);
        cl.expectRevert(bytes("anomaly trigger fired"));
        counter.set(1);

        // Nothing staged this time: the verdict did not carry over.
        cl.assertion(address(counter), _anomalyCode(), ModernAnomalyAssertion.assertNotAnomalous.selector);
        cl.expectRevert(NOT_EXECUTED);
        counter.set(2);
    }

    /// Both ends of the ladder are levels, and neither is rejected.
    function testLadderBoundsAreAccepted() public {
        cl.setAnomalyLevel(address(counter), 1);
        cl.setAnomalyLevel(address(counter), 10);
    }

    /// Past the ladder there is no rung, so the verdict could never be cleared and the assertion
    /// would silently never run, a test passing green having exercised nothing. Rejected at the
    /// cheatcode, where the typo is.
    ///
    /// Called low-level because `expectRevert` only sees calls made at a lower depth than the
    /// cheatcode itself, so it cannot observe a cheatcode rejecting its own argument.
    function testLevelAboveTheLadderIsRejected() public {
        (bool ok,) =
            address(cl).call(abi.encodeWithSignature("setAnomalyLevel(address,uint8)", address(counter), uint8(11)));
        assertTrue(!ok, "a level past the ladder was accepted");

        (bool okTen,) =
            address(cl).call(abi.encodeWithSignature("setAnomalyLevel(address,uint8)", address(counter), uint8(10)));
        assertTrue(okTen, "the loosest rung was rejected");
    }

    function _assertionCode() internal view returns (bytes memory) {
        return abi.encodePacked(type(ModernCounterAssertion).creationCode, abi.encode(counter));
    }

    function _anomalyCode() internal view returns (bytes memory) {
        return abi.encodePacked(type(ModernAnomalyAssertion).creationCode, abi.encode(counter));
    }
}
