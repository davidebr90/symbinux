; Inno Setup script for the Symbinux GUI Windows installer.
; Build order (from the repository root):
;   1. packaging\windows\build-gui.bat
;   2. packaging/windows/make-dist.sh          (MSYS2 MINGW64 shell)
;   3. ISCC.exe packaging\windows\symbinux.iss
; The setup lands in dist\windows\symbinux-gui-<version>-setup-win64.exe

#define MyAppVersion "0.4.0"

[Setup]
AppId={{8E1F4A70-9C2B-4D5E-A6F3-2B7C9D0E1F53}
AppName=Symbinux
AppVersion={#MyAppVersion}
AppPublisher=Davide Pica
AppPublisherURL=https://github.com/davidebr90/symbinux
AppSupportURL=https://github.com/davidebr90/symbinux/issues
DefaultDirName={autopf}\Symbinux
DefaultGroupName=Symbinux
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist\windows
OutputBaseFilename=symbinux-gui-{#MyAppVersion}-setup-win64
SetupIconFile=symbinux.ico
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Per-user install by default (no elevation prompt for an unsigned build);
; the dialog still lets the user pick an all-users install.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
WizardStyle=modern

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "..\..\dist\windows\Symbinux\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion
Source: "symbinux.ico"; DestDir: "{app}"

[Icons]
Name: "{group}\Symbinux"; Filename: "{app}\bin\symbinux-gui.exe"; IconFilename: "{app}\symbinux.ico"
Name: "{autodesktop}\Symbinux"; Filename: "{app}\bin\symbinux-gui.exe"; IconFilename: "{app}\symbinux.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\bin\symbinux-gui.exe"; Description: "{cm:LaunchProgram,Symbinux}"; Flags: nowait postinstall skipifsilent
