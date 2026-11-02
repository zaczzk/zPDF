; zPDF Inno Setup Script
; Zero-dependency native Rust PDF viewer & editor
; Compiles zPDF-Setup-x64.exe or zPDF-Setup-arm64.exe

#ifndef Arch
  #define Arch "x64"
#endif

#define MyAppName "zPDF"
#define MyAppVersion "1.1.2"
#define MyAppPublisher "zPDF"
#define MyAppURL "https://github.com/zaczzk/zpdf"
#define MyAppExeName "zpdf.exe"

[Setup]
AppId={{5C883A7E-873B-492F-A9F7-41E26AE2E651}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequiredOverridesAllowed=dialog commandline
PrivilegesRequired=lowest
OutputDir=.
OutputBaseFilename=zPDF-Setup-{#Arch}
SetupIconFile=assets\logo.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes
; Upgrading in place: reuse the previous folder, privileges and tasks, and let Setup close the
; running app instead of failing on a locked exe.
UsePreviousAppDir=yes
UsePreviousPrivileges=yes
UsePreviousTasks=yes
UsePreviousLanguage=yes
CloseApplications=yes
RestartApplications=no
AllowNoIcons=yes
SetupLogging=yes
VersionInfoVersion={#MyAppVersion}
ArchitecturesInstallIn64BitMode=x64 arm64

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "associatepdf"; Description: "Register zPDF as a handler for PDF files (.pdf)"; GroupDescription: "File Associations:"

[Files]
Source: "stage\zPDF-{#Arch}\zPDF.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "stage\zPDF-{#Arch}\pdfium.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "stage\zPDF-{#Arch}\libunwind.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "LICENSE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Registry]
; ProgID definition
Root: HKA; Subkey: "Software\Classes\zPDF.Document"; ValueType: string; ValueName: ""; ValueData: "zPDF Document"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\zPDF.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\zPDF.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf

; Applications registration
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".pdf"; ValueData: ""; Flags: uninsdeletekey; Tasks: associatepdf

; OpenWithProgids
Root: HKA; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: string; ValueName: "zPDF.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associatepdf

; Windows Default Programs Capabilities registration
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: "Software\{#MyAppName}\Capabilities"; Flags: uninsdeletevalue; Tasks: associatepdf
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Fast, zero-overhead PDF viewer and editor."; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".pdf"; ValueData: "zPDF.Document"; Flags: uninsdeletekey; Tasks: associatepdf

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent


[Code]
; Inno upgrades in place when AppId matches, which is already the case. This adds the two
; behaviours users expect around it: block a downgrade, and ask before reinstalling the same
; version. Anything else is a normal upgrade over the existing install.
const
  UninstallKey = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\{5C883A7E-873B-492F-A9F7-41E26AE2E651}_is1';

function InstalledVersion(): String;
var
  v: String;
begin
  Result := '';
  if RegQueryStringValue(HKCU, UninstallKey, 'DisplayVersion', v) then
    Result := v
  else if RegQueryStringValue(HKLM, UninstallKey, 'DisplayVersion', v) then
    Result := v;
end;

function InitializeSetup(): Boolean;
var
  prev: String;
  cmp: Integer;
begin
  Result := True;
  prev := InstalledVersion();
  if prev = '' then
    Exit;
  cmp := CompareVersion(prev, '{#MyAppVersion}');
  if cmp > 0 then
  begin
    MsgBox('zPDF ' + prev + ' is already installed and is newer than this installer (' + '{#MyAppVersion}' + ').' + #13#10 + #13#10 + 'Uninstall it first if you really want to go backwards.', mbError, MB_OK);
    Result := False;
  end
  else if cmp = 0 then
  begin
    if MsgBox('zPDF ' + prev + ' is already installed.' + #13#10 + #13#10 + 'Reinstall it over the top?', mbConfirmation, MB_YESNO) = IDNO then
      Result := False;
  end;
end;
