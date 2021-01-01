# MC Server Manager

![https://travis-ci.com/FelixResch/mc-server-manager.svg?branch=main](https://travis-ci.com/FelixResch/mc-server-manager.svg?branch=main)

Manager daemon for multiple minecraft servers. Currently only paper servers are supported.

> Currently the daemon only supports starting and stopping servers and listing the currently managed servers. 
> More features are currently being implemented.

## Installation

Build the daemon and put it somewhere it can be run from.

```shell
cargo build --release --bin mcmand
```

Then build the control tool.

```shell
cargo build --release --bin mcman
```

