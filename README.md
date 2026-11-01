# revisor

*English · [Українська](README.uk.md)*

revisor takes snapshots of chosen folders and shows what changed between two snapshots.
Take a snapshot before you install, update or run a program and another one after it, and
revisor lists the files that program created, changed or deleted.

It is a portable Windows application built for the keyboard and for screen readers first.

> **Status: in development.** There is no release yet. The application cannot take snapshots
> from its window so far; this repository holds the work towards version 0.1.

## Building from source

revisor is written in Rust on [wxDragon](https://github.com/AllenDang/wxDragon), which builds
wxWidgets from source as part of `cargo build`. You need, on Windows 10 or 11 (x64):

- [Rust](https://rustup.rs/) with the MSVC toolchain;
- Visual Studio Build Tools with the "Desktop development with C++" workload;
- [CMake](https://cmake.org/) and [Ninja](https://ninja-build.org/) on `PATH`;
- [LLVM](https://llvm.org/), with the `LIBCLANG_PATH` environment variable pointing at its
  `bin` folder (the one that holds `libclang.dll`).

Keep the clone at a short path such as `C:\dev\revisor`: a deep build folder breaks the
wxWidgets build on the Windows path length limit.

```powershell
git clone https://github.com/ruslan-rv-ua/revisor.git
cd revisor
cargo build --release
```

The first build compiles wxWidgets and takes a few minutes; later builds take seconds. The
result is a single executable, `target\release\revisor.exe`, with no DLLs beside it.

Run the tests with `cargo test --workspace`.

## Licence

[MIT](LICENSE). Copyright (c) 2026 Ruslan Iskov.
