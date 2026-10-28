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

; Derive all installer/registration versions from the prepared bundle.
; NSIS !searchparse fails compilation when the version line cannot be read.
; https://nsis.sourceforge.io/Reference/%21searchparse
!searchparse /file "${APP_BUNDLE}\BUILD.json" '"version": "' APP_VERSION '"'

!ifdef PAYROLL_DESKTOP
  !define APP_ID "io.github.tatselkrik.payrolldesktop"
  !define APP_NAME "Payroll Desktop"
  !define APP_EXE "Payroll-Desktop.exe"
  !define APP_MARKER ".payroll-installation"
!else
  !define APP_ID "io.github.tatselkrik.hrisdesktop"
  !define APP_NAME "HRIS Desktop"
  !define APP_EXE "HRIS-Desktop.exe"
  !define APP_MARKER ".hris-installation"
!endif
!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
Name "${APP_NAME}"
OutFile "${SETUP_OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
RequestExecutionLevel user
SetCompressor /SOLID lzma
VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey /LANG=1033 "ProductName" "${APP_NAME}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${APP_VERSION}"
VIAddVersionKey /LANG=1033 "FileDescription" "${APP_NAME} private prototype setup"
VIAddVersionKey /LANG=1033 "FileVersion" "${APP_VERSION}.0"
VIAddVersionKey /LANG=1033 "LegalCopyright" ""

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!insertmacro MUI_PAGE_FINISH
!ifndef UPDATE_ONLY
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!endif
!insertmacro MUI_LANGUAGE "English"

Function .onInit
  SetShellVarContext current
  !ifdef UPDATE_ONLY
    IfFileExists "$INSTDIR\${APP_MARKER}" +3
    SetErrorLevel 2
    Quit
  !endif
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This build requires 64-bit Windows." /SD IDOK
    Abort
  ${EndIf}
  ; Refuse an unfamiliar directory rather than overwriting unrelated files.
  IfFileExists "$INSTDIR\*.*" 0 fresh_install
  FileOpen $0 "$INSTDIR\${APP_MARKER}" r
  IfErrors unfamiliar_install
  FileRead $0 $1
  FileClose $0
  StrCmp $1 "${APP_ID}" own_install unfamiliar_install
fresh_install:
  IfFileExists "$SMPROGRAMS\${APP_NAME}.lnk" unfamiliar_install
  IfFileExists "$DESKTOP\${APP_NAME}.lnk" unfamiliar_install
  Goto own_install
unfamiliar_install:
  MessageBox MB_OK|MB_ICONSTOP "The HRIS installation path or shortcuts already contain unfamiliar files. No files were installed." /SD IDOK
  Abort
own_install:
FunctionEnd

Section "${APP_NAME}"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  SetOverwrite try
  ClearErrors
  File "${APP_BUNDLE}\${APP_EXE}"
  !ifndef UPDATE_ONLY
  File "${APP_BUNDLE}\hris-connection.json"
  !ifndef PAYROLL_DESKTOP
  File "${APP_BUNDLE}\hris-updates.json"
  !endif
  !endif
  File "${APP_BUNDLE}\START-HERE.txt"
  File "${APP_BUNDLE}\VERIFICATION.md"
  !if /FileExists "${APP_BUNDLE}\REVIEW-CHECKLIST.md"
    File "${APP_BUNDLE}\REVIEW-CHECKLIST.md"
  !endif
  File "${APP_BUNDLE}\BUILD.json"
  File "${APP_BUNDLE}\SHA256SUMS.txt"
  IfErrors setup_failed
  !ifdef UPDATE_ONLY
    ReadRegStr $0 HKCU "${UNINSTALL_KEY}" "InstallLocation"
    StrCmp $0 "$INSTDIR" 0 update_done
    WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
    update_done:
    Goto setup_done
  !else
  !if /FileExists "${APP_BUNDLE}\company-logo.png"
    File "${APP_BUNDLE}\company-logo.png"
  !endif
  FileOpen $0 "$INSTDIR\${APP_MARKER}" w
  FileWrite $0 "${APP_ID}"
  FileClose $0
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
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
  FileOpen $0 "$INSTDIR\${APP_MARKER}" r
  IfErrors un_unfamiliar_install
  FileRead $0 $1
  FileClose $0
  StrCmp $1 "${APP_ID}" un_own_install
un_unfamiliar_install:
  MessageBox MB_OK|MB_ICONSTOP "The HRIS installation marker could not be verified. No files were removed." /SD IDOK
  Abort
un_own_install:
FunctionEnd

Section "Uninstall"
  SetShellVarContext current
  ; Remove only this install's named files; retain local settings and unknown files.
  ClearErrors
  Delete "$INSTDIR\${APP_EXE}"
  ${If} ${Errors}
    MessageBox MB_OK|MB_ICONSTOP "Close ${APP_NAME} and run the uninstaller again." /SD IDOK
    Abort
  ${EndIf}
  Delete "$INSTDIR\hris-connection.json"
  !ifndef PAYROLL_DESKTOP
  Delete "$INSTDIR\hris-updates.json"
  !endif
  Delete "$INSTDIR\START-HERE.txt"
  Delete "$INSTDIR\VERIFICATION.md"
  Delete "$INSTDIR\REVIEW-CHECKLIST.md"
  Delete "$INSTDIR\BUILD.json"
  Delete "$INSTDIR\SHA256SUMS.txt"
  Delete "$INSTDIR\company-logo.png"
  Delete "$INSTDIR\${APP_MARKER}"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$SMPROGRAMS\${APP_NAME}.lnk"
  Delete "$DESKTOP\${APP_NAME}.lnk"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
  RMDir "$INSTDIR"
SectionEnd

!endif
