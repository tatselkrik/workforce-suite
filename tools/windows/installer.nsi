; Build with makensis /DAPP_BUNDLE=<absolute bundle directory>
; /DSETUP_OUTPUT=<absolute output exe> tools/windows/installer.nsi
; The bundle is private deployment input, never checked into source control.
Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "x64.nsh"

!ifndef APP_BUNDLE
  !error "APP_BUNDLE must name the prepared Windows bundle"
!endif
!ifndef SETUP_OUTPUT
  !error "SETUP_OUTPUT must name the output installer"
!endif

!define APP_ID "io.github.tatselkrik.hrisdesktop"
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
Name "HRIS Desktop"
OutFile "${SETUP_OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\HRIS Desktop"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "0.1.9.0"
VIAddVersionKey /LANG=1033 "ProductName" "HRIS Desktop"
VIAddVersionKey /LANG=1033 "ProductVersion" "0.1.9"
VIAddVersionKey /LANG=1033 "FileDescription" "HRIS Desktop private prototype setup"
VIAddVersionKey /LANG=1033 "FileVersion" "0.1.9.0"
VIAddVersionKey /LANG=1033 "LegalCopyright" ""

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\HRIS-Desktop.exe"
!insertmacro MUI_PAGE_FINISH
!ifndef UPDATE_ONLY
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!endif
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetShellVarContext current
  !ifdef UPDATE_ONLY
    IfFileExists "$INSTDIR\.hris-installation" +3
    SetErrorLevel 2
    Quit
  !endif
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This build requires 64-bit Windows."
    Abort
  ${EndIf}
  ; Refuse an unfamiliar directory rather than overwriting unrelated files.
  IfFileExists "$INSTDIR\*.*" 0 fresh_install
  FileOpen $0 "$INSTDIR\.hris-installation" r
  IfErrors unfamiliar_install
  FileRead $0 $1
  FileClose $0
  StrCmp $1 "${APP_ID}" own_install unfamiliar_install
fresh_install:
  IfFileExists "$SMPROGRAMS\HRIS Desktop.lnk" unfamiliar_install
  IfFileExists "$DESKTOP\HRIS Desktop.lnk" unfamiliar_install
  Goto own_install
unfamiliar_install:
  MessageBox MB_OK|MB_ICONSTOP "The HRIS installation path or shortcuts already contain unfamiliar files. No files were installed."
  Abort
own_install:
FunctionEnd

Section "HRIS Desktop"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  SetOverwrite try
  ClearErrors
  File "${APP_BUNDLE}\HRIS-Desktop.exe"
  !ifndef UPDATE_ONLY
  File "${APP_BUNDLE}\hris-connection.json"
  File "${APP_BUNDLE}\hris-updates.json"
  !endif
  File "${APP_BUNDLE}\START-HERE.txt"
  File "${APP_BUNDLE}\VERIFICATION.md"
  File "${APP_BUNDLE}\BUILD.json"
  File "${APP_BUNDLE}\SHA256SUMS.txt"
  IfErrors setup_failed
  !ifdef UPDATE_ONLY
    ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
    StrCmp $0 "$INSTDIR" 0 update_done
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "0.1.9"
    update_done:
    Goto setup_done
  !else
  !if /FileExists "${APP_BUNDLE}\company-logo.png"
    File "${APP_BUNDLE}\company-logo.png"
  !endif
  FileOpen $0 "$INSTDIR\.hris-installation" w
  FileWrite $0 "${APP_ID}"
  FileClose $0
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortCut "$SMPROGRAMS\HRIS Desktop.lnk" "$INSTDIR\HRIS-Desktop.exe" "" "$INSTDIR\HRIS-Desktop.exe" 0
  CreateShortCut "$DESKTOP\HRIS Desktop.lnk" "$INSTDIR\HRIS-Desktop.exe" "" "$INSTDIR\HRIS-Desktop.exe" 0
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "HRIS Desktop"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "0.1.9"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\HRIS-Desktop.exe"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" 11100
  !endif
  Goto setup_done
setup_failed:
  SetErrorLevel 2
  Quit
setup_done:
SectionEnd

!ifndef UPDATE_ONLY
Function un.onInit
  SetShellVarContext current
  FileOpen $0 "$INSTDIR\.hris-installation" r
  IfErrors un_unfamiliar_install
  FileRead $0 $1
  FileClose $0
  StrCmp $1 "${APP_ID}" un_own_install
un_unfamiliar_install:
  MessageBox MB_OK|MB_ICONSTOP "The HRIS installation marker could not be verified. No files were removed."
  Abort
un_own_install:
FunctionEnd

Section "Uninstall"
  SetShellVarContext current
  ; Remove only this install's named files; retain local settings and unknown files.
  ClearErrors
  Delete "$INSTDIR\HRIS-Desktop.exe"
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "Close HRIS Desktop and run the uninstaller again."
    Abort
  ${EndIf}
  Delete "$INSTDIR\hris-connection.json"
  Delete "$INSTDIR\hris-updates.json"
  Delete "$INSTDIR\START-HERE.txt"
  Delete "$INSTDIR\VERIFICATION.md"
  Delete "$INSTDIR\BUILD.json"
  Delete "$INSTDIR\SHA256SUMS.txt"
  Delete "$INSTDIR\company-logo.png"
  Delete "$INSTDIR\.hris-installation"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\HRIS Desktop.lnk"
  Delete "$DESKTOP\HRIS Desktop.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  RMDir "$INSTDIR"
SectionEnd

!endif
