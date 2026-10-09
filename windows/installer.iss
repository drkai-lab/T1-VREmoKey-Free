; Inno Setup script for VRChat Expression Hotkeys (Windows 11)

#define AppName "VRChat Expression Hotkeys"
#define AppVersion "1.0.0"
#define AppPublisher "drkai-lab"
#define AppURL "https://github.com/drkai-lab/vrc-expression-hotkeys"

[Setup]
AppId={{A4C8E2B1-7D19-4F6A-9C33-2E5B8A1D0F47}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher={#AppPublisher}
AppPublisherURL={#AppURL}
AppSupportURL={#AppURL}
AppUpdatesURL={#AppURL}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
OutputDir=..\dist-win
OutputBaseFilename=vrc-expression-hotkeys-{#AppVersion}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayIcon={app}\vrc-expression-hotkeys-gui.exe
ArchitecturesInstallIn64BitMode=x64compatible

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start at logon"; GroupDescription: "Autostart"; Flags: unchecked

[Files]
Source: "..\dist-win\vrc-expression-hotkeys.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist-win\vrc-expression-hotkeys-gui.exe"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\{#AppName}"; Filename: "{app}\vrc-expression-hotkeys-gui.exe"
Name: "{group}\Uninstall {#AppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\vrc-expression-hotkeys-gui.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\vrc-expression-hotkeys-gui.exe"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "VRChat Expression Hotkeys"; ValueData: """{app}\vrc-expression-hotkeys-gui.exe"""; Tasks: autostart
