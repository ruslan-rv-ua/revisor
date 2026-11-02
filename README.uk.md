# revisor

*[English](README.md) · Українська*

revisor робить знімки вибраних тек і показує, що змінилося між двома знімками. Зробіть знімок
до встановлення, оновлення чи запуску програми й ще один після, і revisor покаже, які файли
ця програма створила, змінила чи видалила.

Це портативна програма для Windows, яку від початку роблять для роботи з клавіатури й зі
скрінрідером.

> **Стан: у розробці.** Релізу ще немає. Вікно програми вже робить знімки тек профілю, але
> поки що не вміє їх порівнювати; у цьому репозиторії триває робота над версією 0.1.

## Збірка із сирців

revisor написано на Rust із [wxDragon](https://github.com/AllenDang/wxDragon), який збирає
wxWidgets із сирців під час `cargo build`. Потрібні Windows 10 чи 11 (x64) і:

- [Rust](https://rustup.rs/) з тулчейном MSVC;
- Visual Studio Build Tools із навантаженням «Desktop development with C++»;
- [CMake](https://cmake.org/) і [Ninja](https://ninja-build.org/) у `PATH`;
- [LLVM](https://llvm.org/) і змінна середовища `LIBCLANG_PATH` з його текою `bin` (тією, де
  лежить `libclang.dll`).

Клонуйте репозиторій у короткий шлях, наприклад `C:\dev\revisor`: глибока тека збірки ламає
збірку wxWidgets через обмеження довжини шляху Windows.

```powershell
git clone https://github.com/ruslan-rv-ua/revisor.git
cd revisor
cargo build --release
```

Перша збірка компілює wxWidgets і триває кілька хвилин, наступні — секунди. Результат — один
файл `target\release\revisor.exe` без DLL поруч.

Тести запускає `cargo test --workspace`.

## Ліцензія

[MIT](LICENSE). Copyright (c) 2026 Ruslan Iskov.
