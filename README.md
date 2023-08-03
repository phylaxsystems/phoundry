# Fork of Foundry

This is a modified version of [Foundry](https://github.com/foundry-rs/foundry) used by Phylax.

## Changes

- Commented out codepaths for ledger, etherscan that were not `Send`, as they rendered `TestArgs` not `Send` as well. They are not used by Phylax anyway
- `vm.export(string, string)` cheatcode to make data available to the `TestOutcome` struct, which is then consumed by Phylax
