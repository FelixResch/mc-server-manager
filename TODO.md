## Functionality

- [ ] Implement daemon with `async-std`
- [ ] Represent clients with custom type `Client` instead of `u32`
- [x] Move unit config to `<unit_name>.server` files (currently uses `*.toml` files)
- [ ] Use a different approach to load units
  - [ ] support different types of units
    - [x] servers
    - [ ] caches (of repos)
    - [ ] repositories
  - [ ] load units with different file extensions (e.g. `*.server`)
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
- [ ] Commands 
  - [ ] `list versions`: lists available versions of a server type
  - [ ] `list builds`: list available builds for a version of a server type (where applicable)
- [ ] Internationalize controller (eventually)

## Code

- [ ] Proper code documentation
- [ ] Proper error handling (no `.unwrap()`, error types and at places `.expect(..)`)
- [ ] Check used paradigms
- [ ] Include `rustfmt` and `clippy` (even if it's annoying)

## General

- [ ] **TESTS!!!**
- [x] Travis CI
- [ ] Quick Start guide
- [ ] Documentation for mortals
