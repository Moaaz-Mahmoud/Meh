; Inno Setup 6 script for meh (D-09, revised by D-18).
;
; By default it installs for all users (Program Files, the all-users Start
; Menu and Startup folders) and needs administrator rights. On PCs with
; application control, exes may only run from Program Files, and setup
; itself can only be started with "Run as administrator", often as a
; separate admin account. A per-user install there would land in that
; admin account's profile, so all-users is the default. The first page
; still offers "Install for me only" (no UAC prompt) for unmanaged PCs.
;
; Build (from the repo root, after `cargo build --release`):
;   ISCC.exe /DMyAppVersion=0.1.0 installer\meh.iss
; Output: dist\meh-<version>-setup.exe
;
; The uninstaller removes the program files, the Start Menu entry and the
; Startup shortcut. It leaves every user's snippets in %APPDATA%\meh alone.

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
; All users by default; "Install for me only" is offered on the first page
; (and as /CURRENTUSER; `dialog` also allows the command line). {autopf} is
; Program Files for all users and %LOCALAPPDATA%\Programs for one user.
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog commandline
; Do not silently reuse an earlier install's mode: an earlier per-user
; install made by an admin account (G4-1) would otherwise keep every
; upgrade in that account's profile.
UsePreviousPrivileges=no
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
Name: "autostart"; Description: "Start Meh when you sign in to Windows"; GroupDescription: "Startup:"

[Files]
Source: "..\target\release\meh.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
; {autoprograms} and {autostartup} are the all-users Start Menu and Startup
; folders in an all-users install, and the user's own in a per-user one.
; Shortcuts there start meh as whoever signs in, never as the admin account
; that ran setup (D-18). The uninstaller removes both. An install without
; the autostart task (or a reinstall that unticks it) leaves no shortcut.
Name: "{autoprograms}\Meh"; Filename: "{app}\meh.exe"; Comment: "Meh text expander"
Name: "{autostartup}\Meh"; Filename: "{app}\meh.exe"; Comment: "Meh text expander"; Tasks: autostart

[InstallDelete]
; A reinstall with the autostart task unticked removes an earlier shortcut.
Type: files; Name: "{autostartup}\Meh.lnk"; Tasks: not autostart

[Run]
; Offered only in a per-user install. In an all-users install setup runs as
; an administrator, often a separate admin account, and meh would run as
; that account and use its profile (D-18); the finish message tells the
; user to start meh from the Start Menu instead. runasoriginaluser is
; already the default for postinstall entries; it is spelled out because
; starting meh as the wrong account is the bug this guards against.
Filename: "{app}\meh.exe"; Description: "Start Meh now"; Flags: nowait postinstall skipifsilent runasoriginaluser; Check: not IsAdminInstallMode

[Code]
// Setup runs as an administrator (for example "Run as administrator" with a
// separate admin account) but the user chose "Install for me only": meh
// would go to that admin account's profile, not the signed-in user's
// (G4-1, D-18). Ask before going on. Never asked in silent mode. The
// install mode is chosen on the first dialog, before this runs. A user who
// is an administrator without UAC filtering may see this for their own
// account; Yes goes on as before.
function InitializeSetup(): Boolean;
begin
  Result := True;
  if IsAdmin() and (not IsAdminInstallMode()) and (not WizardSilent()) then
    Result := MsgBox('Setup is running as an administrator account but installing just for that account, so Meh would not be available to you.' + #13#10#13#10 +
      'To install it for yourself, choose No, then run setup again and choose "Install for all users".' + #13#10#13#10 +
      'Continue anyway?', mbConfirmation, MB_YESNO) = IDYES;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    // Earlier versions autostarted through an HKCU Run value. Remove it
    // from the hive of the account running setup (best effort; another
    // user's hive is not reachable here), so the Startup shortcut is the
    // only autostart.
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'meh');
    if IsAdminInstallMode() and (not WizardSilent()) then
      MsgBox('Meh was installed for all users.' + #13#10#13#10 +
        'Start it from the Start Menu as yourself (not as administrator). It will also start automatically when you sign in.',
        mbInformation, MB_OK);
  end;
end;
