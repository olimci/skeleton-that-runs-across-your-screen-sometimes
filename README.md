# nixos service that makes a skeleton run across your screen sometimes

as described in the title.

## usage

### with nixos

add the repo as an input:

```nix
inputs.skeleton.url = "github:olimci/skeleton-that-runs-across-your-screen-sometimes";
```

then enable:

```nix
imports = [ inputs.skeleton.nixosModules.default ];
services.skeleton-that-runs-across-your-screen-sometimes.enable = true;
```

### standalone

build:

```sh
nix build .#default
```

run:

```sh
./result/bin/skeleton-that-runs-across-your-screen-sometimes
```
