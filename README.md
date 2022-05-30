# CosmWasm CTF 

CosmWasm CTF (or `CW-CTF` for short) is a repository that consists of intentionally written vulnerable CosmWasm smart contracts. You can think of this as a CosmWasm version of [OpenZeppelin Ethernaut](https://github.com/OpenZeppelin/ethernaut) challenge. While the challenges here will be as native CosmWasm as possible (*i.e.*, bugs that would only appear in the CW environment), it would be beneficial to add other challenges that work across different blockchain environments such as Solidity or Solana ecosystems.

Please **do not** use this in production.

## Challenges

Every challenge in the repo comes with an associated proof of concept (`POC`) exploit code. The exploit code is written as test cases with their test case name as `exploit()`. Most of the time, you can reproduce the vulnerability by running the following command:

```bash
cargo test
```

Having a proof of concept exploit code will give you an idea of how the contract is vulnerable and how an attacker can exploit it, which can serve as a good learning opportunity. 

## Contributing

Did you happen to find an unintended bug in the challenge? Or have a new vulnerable contract you want to share? Or maybe just a simple efficiency suggestion? Feel free to open an issue or submit a pull request! Please keep in mind that for now, only bugs with an exploitable scenario and an accompanying proof of concept exploit code will be accepted.

## Good to know
- If you're entirely new to Rust or CosmWasm, the [Terra Academy](https://academy.terra.money/collections) is an excellent tutorial to pick up the basics.
- For simplicity, the UST denomination used in the contract assumes 0 decimals.
