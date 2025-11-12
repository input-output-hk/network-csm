# Cardano Network Webapp

This is a cardano webapp template. It uses [`yew`] for the web rendering part.

The example connect to the cardano network and display the latest known tip.

## Live demo

TBD

## How to run

To run use the template you will need to use [`Trunk`]. See [`Trunk`]
website for installation on your platform.

First you need to run the local WebSocket proxy:

```
cargo run --bin network-csm-cardano-ws-proxy
```

Then you can run the handy script:

```
./run.sh
```

[`Trunk`]: https://trunkrs.dev
[`yew`]: https://yew.rs
