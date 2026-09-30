#ifndef AppVersion
  #error Pass /DAppVersion=<Cargo.toml version> to ISCC.
#endif

[Setup]
AppId={{400e155e-7e8c-4f36-89c5-5a96103744fc}
AppName=GitVibe
AppVersion={#AppVersion}
AppPublisher=GitVibe Contributors
AppPublisherURL=https://github.com/boubou666/GitVibe
AppSupportURL=https://github.com/boubou666/GitVibe/issues
AppUpdatesURL=https://github.com/boubou666/GitVibe/releases
DefaultDirName={localappdata}\Programs\GitVibe
DefaultGroupName=GitVibe
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
OutputDir=..\dist
OutputBaseFilename=GitVibe-windows-X64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern dark
SetupIconFile=..\assets\gitvibe.ico
WizardImageFile=..\assets\installer-banner.png
WizardSmallImageFile=..\assets\gitvibe-icon.png
CloseApplications=yes
RestartApplications=no
UninstallDisplayIcon={app}\gitvibe.exe
LicenseFile=..\LICENSE

[Files]
Source: "..\target\release\gitvibe.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked

[Icons]
Name: "{autoprograms}\GitVibe"; Filename: "{app}\gitvibe.exe"
Name: "{autodesktop}\GitVibe"; Filename: "{app}\gitvibe.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\gitvibe.exe"; Description: "Launch GitVibe"; Flags: nowait postinstall skipifsilent
