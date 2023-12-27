// SPDX-License-Identifier: Unlicense
pragma solidity 0.8.18;

import "ds-test/test.sol";
import "./Vm.sol";

contract PhylaxTest is DSTest {
    Vm constant vm = Vm(HEVM_ADDRESS);

    function test_importContext {
        bytes memory raw = vm.importContext("test_key");
        string memory value = abi.decode(raw, (string));
        assertEq(value, "");
    }
}
