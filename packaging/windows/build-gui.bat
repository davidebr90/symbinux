@echo off
rem Build the Symbinux GUI natively on Windows against the MSYS2 GTK4 runtime.
rem
rem One-time prerequisites:
rem   winget install MSYS2.MSYS2
rem   C:\msys64\usr\bin\pacman -S --needed --noconfirm ^
rem       mingw-w64-x86_64-gtk4 mingw-w64-x86_64-toolchain mingw-w64-x86_64-pkgconf
rem   rustup target add x86_64-pc-windows-gnu
rem
rem Run from the repository root. The binary lands in
rem   target\x86_64-pc-windows-gnu\release\symbinux-gui.exe

setlocal
set "MSYS2=C:\msys64"
set "PATH=%MSYS2%\mingw64\bin;%PATH%"
set "PKG_CONFIG=%MSYS2%\mingw64\bin\pkgconf.exe"
set "PKG_CONFIG_PATH=%MSYS2%\mingw64\lib\pkgconfig"
set "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=gcc"

cargo build --release -p symbinux-gui --target x86_64-pc-windows-gnu
endlocal
