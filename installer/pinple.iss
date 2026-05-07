; 핀플 PC 앱 Inno Setup 스크립트
; GitHub Actions 에서는 다음 매개변수가 자동 주입됨:
;   /DAppVersion=<git tag>
;   /DSourceExe=<absolute path to pinple_pc_agent.exe>

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

#ifndef SourceExe
  #define SourceExe "..\target\x86_64-pc-windows-msvc\release\pinple_pc_agent.exe"
#endif

[Setup]
AppId={{B6A4A9D2-9F1C-4B5C-8E0E-CC1B7D5E2F00}
AppName=핀플 PC
AppVersion={#AppVersion}
AppPublisher=Pinple
DefaultDirName={pf}\Pinple\PCAgent
DefaultGroupName=핀플 PC
DisableDirPage=no
DisableProgramGroupPage=yes
OutputDir=Output
OutputBaseFilename=PinplePCAgent_Setup_{#AppVersion}
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64
PrivilegesRequired=admin
WizardStyle=modern
UninstallDisplayName=핀플 PC

[Languages]
Name: "korean"; MessagesFile: "compiler:Languages\Korean.isl"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; Flags: ignoreversion
; 아이콘이 있으면 함께 포함 (없어도 빌드 통과).
Source: "..\resources\icon.ico"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\핀플 PC"; Filename: "{app}\pinple_pc_agent.exe"
Name: "{group}\{cm:UninstallProgram,핀플 PC}"; Filename: "{uninstallexe}"
Name: "{commondesktop}\핀플 PC"; Filename: "{app}\pinple_pc_agent.exe"; Tasks: desktopicon

[Tasks]
Name: "desktopicon"; Description: "바탕화면 바로가기 만들기"; GroupDescription: "추가 작업:"; Flags: checkedonce
Name: "autostart"; Description: "윈도우 시작 시 자동 실행"; GroupDescription: "추가 작업:"; Flags: checkedonce

[Registry]
; 자동 실행 (HKCU 기준 — 사용자별 등록)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
      ValueType: string; ValueName: "PinplePCAgent"; \
      ValueData: """{app}\pinple_pc_agent.exe"""; \
      Flags: uninsdeletevalue; Tasks: autostart

[Run]
Filename: "{app}\pinple_pc_agent.exe"; Description: "핀플 PC 시작"; \
         Flags: nowait postinstall skipifsilent
