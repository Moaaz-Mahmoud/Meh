; Inno Setup 6 script for meh: a per-user install, no UAC prompt (D-09).
;
; Build (from the repo root, after `cargo build --release`):
;   ISCC.exe /DMyAppVersion=0.1.0 installer\meh.iss
; Output: dist\meh-<version>-setup.exe
;
; The uninstaller removes the program files, the Start Menu entry and the
; autostart value. It leaves the user's snippets in %APPDATA%\meh alone.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0-dev"
#endif

[Setup]
; Fixed forever: Windows uses it to recognise upgrades and the uninstaller.
; The leading "{{" is Inno's escape for a literal "{".
AppId={{A53C1CA6-9509-4475-ACE2-525913C11815}
AppName=Meh
AppVersion={#MyAppVersion}
AppVerName=Meh {#MyAppVersion}
AppPublisher=Moaaz Mahmoud
AppPublisherURL=https://github.com/Moaaz-Mahmoud/meh
AppSupportURL=https://github.com/Moaaz-Mahmoud/meh/issues
; Per-user: {autopf} resolves to %LOCALAPPDATA%\Programs.
PrivilegesRequired=lowest
DefaultDirName={autopf}\meh
DisableProgramGroupPage=yes
; Must match MUTEX_NAME in src/win/single_instance.rs. Setup and the
; uninstaller ask the user to quit a running meh first.
AppMutex=meh-single-instance
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=meh-{#MyAppVersion}-setup
SetupIconFile=..\assets\meh.ico
UninstallDisplayIcon={app}\meh.exe
UninstallDisplayName=Meh
WizardStyle=modern
Compression=lzma2
SolidCompression=yes

[Tasks]
; On by default (D-09): an expander that is not running is no use.
Name: "autostart"; Description: "Start Meh when I sign in to Windows"; GroupDescription: "Startup:"

[Files]
Source: "..\target\release\meh.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Meh"; Filename: "{app}\meh.exe"; Comment: "Meh text expander"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "meh"; ValueData: """{app}\meh.exe"""; Flags: uninsdeletevalue; Tasks: autostart
; Unticking the box on a reinstall removes an earlier autostart value.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "meh"; Flags: deletevalue; Tasks: not autostart

[Run]
Filename: "{app}\meh.exe"; Description: "Start Meh now"; Flags: nowait postinstall skipifsilent
