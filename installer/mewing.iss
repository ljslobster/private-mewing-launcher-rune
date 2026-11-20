; Rune standalone installer. scripts/build_installer.ps1 supplies AppVersion
; and dist\PrivateMewingLauncher.exe. Application assets/fonts are embedded.
#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

#define AppName "Private Mewing Launcher"
#define AppExeName "PrivateMewingLauncher"
#define AppPublisher "Private Mewing"
#define AppDescription "Private Minecraft Launcher"

[Setup]
; Keep the original Flet AppId so Rune upgrades the same installation.
AppId={{8E4C1F62-9B3A-4D57-9C21-5F0A7D3B6E14}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher={#AppPublisher}
AppComments={#AppDescription}
VersionInfoVersion={#AppVersion}
VersionInfoProductName={#AppName}
VersionInfoProductVersion={#AppVersion}
VersionInfoCompany={#AppPublisher}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\{#AppExeName}.exe
OutputDir=..\dist
OutputBaseFilename={#AppExeName}-Setup-{#AppVersion}
SetupIconFile=..\assets\icon.ico
WizardStyle=modern
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
MinVersion=10.0

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Files]
; Only the compiled application is needed; no Python runtime or asset tree.
Source: "..\dist\{#AppExeName}.exe"; DestDir: "{app}"; Flags: ignoreversion
; Retain the original shortcut icon even when the native exe has no resource icon.
Source: "..\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\{#AppExeName}.exe"; IconFilename: "{app}\icon.ico"
Name: "{group}\{cm:UninstallProgram,{#AppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExeName}.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\{#AppExeName}.exe"; Description: "{cm:LaunchProgram,{#StringChange(AppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
