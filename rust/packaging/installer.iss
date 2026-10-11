; Inno Setup script for the Rust free edition (Windows x64)

#define AppName "T1-VREmoKey-Free"
#define AppVersion "2.0.3"
#define AppPublisher "drkai-lab"
#define AppURL "https://github.com/drkai-lab/T1-VREmoKey-Free"

[Setup]
AppId={{B7E2C4A9-1F58-4D0E-9A66-3C8B5E2D7A10}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputDir=..\..\dist-win-rust
OutputBaseFilename=vrc-expression-hotkeys-{#AppVersion}-setup
SetupIconFile=..\assets\app.ico
UninstallDisplayIcon={app}\vrc-expression-hotkeys-gui.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start at logon"; GroupDescription: "Autostart"; Flags: unchecked

[Files]
Source: "..\target\release\vrc-expression-hotkeys.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\vrc-expression-hotkeys-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\OFL"; DestDir: "{app}"; DestName: "OFL.txt"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\vrc-expression-hotkeys-gui.exe"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\vrc-expression-hotkeys-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\vrc-expression-hotkeys-gui.exe"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "{#AppName}"; ValueData: """{app}\vrc-expression-hotkeys-gui.exe"""; Tasks: autostart; Flags: uninsdeletevalue
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "VRChat Expression Hotkeys"; Flags: deletevalue
