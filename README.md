# CosmWasm CTF 

CosmWasm CTF (or `CW-CTF` for short) is a repository that consists of intentionally written vulnerable CosmWasm smart contracts. You can think of this as a CosmWasm version of [OpenZeppelin Ethernaut](https://github.com/OpenZeppelin/ethernaut) challenge. While the challenges here will be as native CosmWasm as possible (*i.e.*, bugs that would only appear in the CW environment), it would be beneficial to add other challenges that work across different blockchain environments such as Solidity or Solana ecosystems.

Please do not use this in production.

## Challenges

Every challenge in the repo comes with an associated proof of concept (`POC`) exploit code. The exploit code is written as test cases with their test case name as `exploit()`. Most of the time, you can reproduce the vulnerability by running the following command:

```bash
cargo test
```

Having a proof of concept exploit code will give you an idea of how the contract is vulnerable and how an attacker can exploit it, which can serve as a good learning opportunity. 
