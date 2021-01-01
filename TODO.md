## Functionality

- [ ] Implement daemon with `async-std`
- [ ] Represent clients with custom type `Client` instead of `u32`
- [ ] Move unit config to `<unit_name>.server` files
- [ ] Create the possibility to create & update servers for 
    - [ ] PaperMC
    - [ ] Spigot
    - [ ] Bukkit
    - [ ] Vanilla (?)
- [ ] Create the possibility to install/update/remove plugins for
    - [ ] PaperMC
    - [ ] Spigot
    - [ ] Bukkit
- [ ] Add CLI/Web GUI for management
- [ ] Allow the config location to be set for the daemon

## Code

- [ ] Proper code documentation
- [ ] Proper error handling (no `.unwrap()`, error types and at places `.expect(..)`)
- [ ] Check used paradigms
- [ ] Include `rustfmt` and `clippy` (even if it's annoying)

## General

- [ ] **TESTS!!!**
- [ ] Travis CI
- [ ] Quick Start guide
- [ ] Documentation for mortals
