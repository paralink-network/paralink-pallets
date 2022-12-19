# Paralink Feeds pallet

The pallet allows submitting oracle values into the feed defined by the IPFS hash. The given feed is periodically read and executed by the [oracle paralink-node](https://github.com/paralink-network/paralink-node) hosted by the owner of the feed.


## Usage
1. The feed creator has to be registered in the Paralink Substrate Node by the administrator.
2. The feed creator creates a PQL definition on the [IPFS](https://github.com/paralink-network/paralink-node/blob/master/examples/ipfs_request.py#L12).
3. The feed creator has to have at least `FeedStakingBalance` balance in the wallet (defined by the Substrate node) to be able to create the feed.
4. The feed creator sets up the [oracle paralink-node](https://github.com/paralink-network/paralink-node) and registers the feed with the same account that is owner of the feed.
5. The `paralink-node` will periodically execute the IPFS hash and submit the value to the Paralink Substrate Node as long it holds at least `FeedStakingBalance` assets. 


## Cross-chain price updates (XCM)
A consumer chain on the same relay chain as Paralink Substrate Node can then start receiving the price updates as long as it implements the [paralink-xcm pallet](https://github.com/paralink-network/paralink-pallets/tree/master/xcm) and has enough assets (defined by `FeedStakingBalance`).

The price updates can then be used by the [ink! contract pallets](https://github.com/paralink-network/paralink-pallets/tree/master/ink-extension) if the pallet is integrated into the consumer chain or by any other pallet.
See [paralink-xcm repo](https://github.com/paralink-network/paralink-xcm) for a demo example.
