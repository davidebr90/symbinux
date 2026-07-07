# Windows packaging

Produces a self-contained, double-clickable build of the Symbinux GUI: the
GTK4 runtime DLLs travel with the executable and the GUI itself defaults
`GSK_RENDERER` to cairo on Windows, so no environment setup is required on the
target machine.

## Prerequisites (build machine, one-time)

```bat
winget install MSYS2.MSYS2
C:\msys64\usr\bin\pacman -S --needed --noconfirm ^
    mingw-w64-x86_64-gtk4 mingw-w64-x86_64-toolchain mingw-w64-x86_64-pkgconf
rustup target add x86_64-pc-windows-gnu
```

For the installer, additionally: [Inno Setup 6](https://jrsoftware.org/isinfo.php)
(`winget install JRSoftware.InnoSetup`).

## Build steps (from the repository root)

1. `packaging\windows\build-gui.bat` — release build against MSYS2 GTK4
   (`x86_64-pc-windows-gnu`).
2. `packaging/windows/make-dist.sh` from an **MSYS2 MINGW64 shell** — assembles
   `dist/windows/Symbinux/` with the exe, the mingw64 DLL closure (via `ldd`),
   gdk-pixbuf loaders, compiled GSettings schemas, Adwaita/hicolor icons, the
   theme-aware logos and the runtime `.po` translations.
3. `ISCC.exe packaging\windows\symbinux.iss` — compiles the installer to
   `dist\windows\symbinux-gui-<version>-setup-win64.exe`.

The `dist/windows/Symbinux/` folder is also usable as a portable app: run
`bin\symbinux-gui.exe` directly.

## Notes

- The MSYS2 mingw64 runtime matches the `x86_64-pc-windows-gnu` Rust target
  (MSVCRT); do not mix it with MSVC builds of the GUI.
- Raw-USB access on Windows requires the WinUSB driver on the device
  (see `docs/CROSS_PLATFORM.md`); serial/COM detection works out of the box.
- Binaries are unsigned; SmartScreen may warn on first launch. Code signing is
  a separate, later decision.
