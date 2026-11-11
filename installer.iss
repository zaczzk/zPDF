; zPDF Inno Setup Script
; Zero-dependency native Rust PDF viewer & editor
; Compiles zPDF-Setup-x64.exe or zPDF-Setup-arm64.exe

#ifndef Arch
  #define Arch "x64"
#endif

; The version comes from Cargo.toml, passed by CI: ISCC /DArch=x64 /DMyAppVersion=1.2.3 installer.iss
#ifndef MyAppVersion
  #error MyAppVersion is not defined. Pass /DMyAppVersion=<version from Cargo.toml> to ISCC.
#endif

#define MyAppName "zPDF"
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
; Must match src/default_app.rs (register_at), which writes the same keys for `zpdf --register`.
; The .pdf default value is deliberately NOT written: the user picks the default in Settings.
; ProgID definition
Root: HKA; Subkey: "Software\Classes\zPDF.Document"; ValueType: string; ValueName: ""; ValueData: "zPDF Document"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\zPDF.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\zPDF.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf

; Applications registration
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\zPDF.Document"; ValueType: string; ValueName: "FriendlyTypeName"; ValueData: "zPDF Document"; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"",0"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\Classes\Applications\{#MyAppExeName}\SupportedTypes"; ValueType: string; ValueName: ".pdf"; ValueData: ""; Flags: uninsdeletekey; Tasks: associatepdf

; Offer zPDF in "Open with" for .pdf
Root: HKA; Subkey: "Software\Classes\.pdf\OpenWithProgids"; ValueType: string; ValueName: "zPDF.Document"; ValueData: ""; Flags: uninsdeletevalue; Tasks: associatepdf
; Windows Default Programs Capabilities registration
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#MyAppName}"; ValueData: "Software\{#MyAppName}\Capabilities"; Flags: uninsdeletevalue; Tasks: associatepdf
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "Fast, zero-overhead PDF viewer and editor."; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey; Tasks: associatepdf
Root: HKA; Subkey: "Software\{#MyAppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".pdf"; ValueData: "zPDF.Document"; Flags: uninsdeletekey; Tasks: associatepdf
; Per-user settings written by the app (NeverAskDefault): removed on uninstall.
Root: HKCU; Subkey: "Software\{#MyAppName}"; Flags: uninsdeletekey

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Removes per-user registrations the app made itself (in-app "Set default" banner or
; `zpdf --register`), e.g. when the associatepdf task was left unchecked. Registry only.
Filename: "{app}\{#MyAppExeName}"; Parameters: "--unregister"; Flags: runhidden waituntilterminated; RunOnceId: "zPDFUnregister"


[Code]
// Inno upgrades in place when AppId matches, which is already the case. This adds a heads-up
// when the machine already has zPDF: a downgrade is never blocked, only confirmed, and the same
// version asks before being reinstalled. A real upgrade installs without asking.
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

// CompareVersion is Delphi-side only; Inno's Pascal Script has no such function.
function NextVerField(var S: String): Integer;
var
  P: Integer;
begin
  P := Pos('.', S);
  if P > 0 then
  begin
    Result := StrToIntDef(Copy(S, 1, P - 1), 0);
    S := Copy(S, P + 1, Length(S));
  end
  else
  begin
    Result := StrToIntDef(S, 0);
    S := '';
  end;
end;

function CompareVer(A, B: String): Integer;
var
  I, NA, NB: Integer;
begin
  Result := 0;
  for I := 1 to 4 do
  begin
    NA := NextVerField(A);
    NB := NextVerField(B);
    if NA < NB then
    begin
      Result := -1;
      Exit;
    end;
    if NA > NB then
    begin
      Result := 1;
      Exit;
    end;
  end;
end;

function AskConfirm(const Text: String; const SilentAnswer: Integer): Integer;
begin
  Result := SuppressibleMsgBox(Text, mbConfirmation, MB_YESNO or MB_DEFBUTTON2, SilentAnswer);
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
  cmp := CompareVer(prev, '{#MyAppVersion}');
  // SuppressibleMsgBox answers with the Default we give it when the install runs silently
  // (/SILENT or /VERYSILENT). A plain MsgBox here would block a silent run forever.
  if cmp > 0 then
  begin
    // Downgrade: allowed, never blocked. AskConfirm() passes MB_DEFBUTTON2, so the second
    // button (No) is preselected for a human -- pressing Enter declines. The 4th argument is
    // only used when the install runs silently (/SILENT, /VERYSILENT), and it is Yes, so a
    // silent run never blocks and never hangs; silent runs are unattended by definition.
    Log('zPDF downgrade offered: installed ' + prev + ', installer ' + '{#MyAppVersion}');
    if AskConfirm('This machine already has zPDF ' + prev + ', which is newer than this installer (' + '{#MyAppVersion}' + ').' + #13#10 + #13#10 + 'Install the older version anyway?', IDYES) = IDNO then
    begin
      Log('zPDF downgrade declined by the user');
      Result := False;
    end
    else
      Log('zPDF downgrade accepted; installing ' + '{#MyAppVersion}' + ' over ' + prev);
  end
  else if cmp = 0 then
  begin
    if AskConfirm('zPDF ' + prev + ' is already installed.' + #13#10 + #13#10 + 'Reinstall it over the top?', IDYES) = IDNO then
      Result := False;
  end;
end;
