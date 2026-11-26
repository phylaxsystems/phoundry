---
foundry-cheatcodes: patch
forge: patch
---

Raised the local Credible assertion test gas cap from 300,000 to 3,000,000 gas, matching the executor's default budget. Assertions at the cap pass; assertions above it still fail. Production budgets remain runtime-configured.
