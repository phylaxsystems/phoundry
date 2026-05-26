// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.18;

import "ds-test/test.sol";
import "cheats/Vm.sol";
import {Assertion} from "credible-std/Assertion.sol";

contract MockAssertion is Assertion {
    MockContract mockContract;

    constructor(address mockContract_) {
        mockContract = MockContract(mockContract_);
    }

    function fnSelectors() external pure override returns (bytes4[] memory selectors) {
        selectors = new bytes4[](2);
        selectors[0] = this.assertIsOne.selector;
        selectors[1] = this.assertIsTwo.selector;
    }

    function assertIsOne() external view returns (bool) {
        return mockContract.value() == 1;
    }

    function assertIsTwo() external view returns (bool) {
        return mockContract.value() == 2;
    }
}

contract MockContract {
    uint256 public value = 1;

    function increment() public {
        value++;
    }
}

contract CredibleTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    address assertionAdopter;

    address constant caller = address(0xdead);

    function setUp() public {
        assertionAdopter = address(new MockContract());
        vm.deal(caller, 1 ether);
    }

    function testAssertionPass() public {
        emit log_address(assertionAdopter);

        bytes memory assertion = abi.encodePacked(type(MockAssertion).creationCode, abi.encode(assertionAdopter));

        vm.assertion(assertionAdopter, assertion, MockAssertion.assertIsOne.selector);
        assertTrue(MockContract(assertionAdopter).value() == 1);

        MockContract(assertionAdopter).increment();
        assertTrue(MockContract(assertionAdopter).value() == 2);

        vm.assertion(assertionAdopter, assertion, MockAssertion.assertIsTwo.selector);
        assertTrue(MockContract(assertionAdopter).value() == 2);
    }
}
