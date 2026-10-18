; KnuPdf Windows installer (NSIS, built on Linux with makensis).
; makensis -DVERSION=0.8.0 -DSTAGE=<staging dir> -DOUT=<installer.exe> knupdf.nsi
; The staging dir holds KnuPdf.exe, models\, the licences and the app icon (KnuPdf.ico).

Unicode true
SetCompressor /SOLID lzma
RequestExecutionLevel admin
ManifestDPIAware true

!include "MUI2.nsh"
!include "x64.nsh"

!define APP "KnuPdf"
!define EXE "KnuPdf.exe"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP}"

Name "${APP}"
OutFile "${OUT}"
InstallDir "$PROGRAMFILES64\${APP}"
InstallDirRegKey HKLM "Software\${APP}" "InstallDir"
BrandingText "${APP} ${VERSION}"
VIProductVersion "${VERSION}.0"
VIAddVersionKey /LANG=1042 "ProductName" "${APP}"
VIAddVersionKey /LANG=1042 "CompanyName" "강원대학교"
VIAddVersionKey /LANG=1042 "FileDescription" "${APP} 설치 프로그램"
VIAddVersionKey /LANG=1042 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1042 "LegalCopyright" "Based on PdfCraft. MIT OR Apache-2.0."

!define MUI_ICON "${STAGE}\KnuPdf.ico"
!define MUI_UNICON "${STAGE}\KnuPdf.ico"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "KnuPdf 실행"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE}\LICENSE-KnuPdf.txt"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "Korean"

Function .onInit
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "KnuPdf는 64비트 Windows에서만 설치할 수 있습니다."
    Abort
  ${EndIf}
  SetRegView 64
FunctionEnd

Function un.onInit
  SetRegView 64
FunctionEnd

Section "KnuPdf (필수)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"
  File "${STAGE}\${EXE}"
  File "${STAGE}\KnuPdf.ico"
  File "${STAGE}\LICENSE-KnuPdf.txt"
  File "${STAGE}\LICENSE-MIT"
  File "${STAGE}\LICENSE-APACHE"
  File "${STAGE}\NOTICE"
  File "${STAGE}\LICENSE-icon.txt"
  File "${STAGE}\OFL-KnupdfHangul.txt"
  SetOutPath "$INSTDIR\models"
  File /r "${STAGE}\models\*.*"
  SetOutPath "$INSTDIR"

  WriteUninstaller "$INSTDIR\uninstall.exe"
  WriteRegStr HKLM "Software\${APP}" "InstallDir" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayName" "${APP}"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "${UNINST_KEY}" "Publisher" "강원대학교"
  WriteRegStr HKLM "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE}"
  WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKLM "${UNINST_KEY}" "NoRepair" 1

  ; Offer KnuPdf in "Open with" for PDFs without taking over the default PDF app.
  WriteRegStr HKLM "Software\Classes\KnuPdf.Document" "" "PDF 문서"
  WriteRegStr HKLM "Software\Classes\KnuPdf.Document\DefaultIcon" "" "$INSTDIR\${EXE},-2"
  WriteRegStr HKLM "Software\Classes\KnuPdf.Document\shell\open\command" "" '"$INSTDIR\${EXE}" "%1"'
  WriteRegStr HKLM "Software\Classes\.pdf\OpenWithProgids" "KnuPdf.Document" ""
  WriteRegStr HKLM "Software\Classes\Applications\${EXE}\shell\open\command" "" '"$INSTDIR\${EXE}" "%1"'
  WriteRegStr HKLM "Software\Classes\Applications\${EXE}\SupportedTypes" ".pdf" ""

  CreateDirectory "$SMPROGRAMS\${APP}"
  CreateShortcut "$SMPROGRAMS\${APP}\${APP}.lnk" "$INSTDIR\${EXE}"
  CreateShortcut "$SMPROGRAMS\${APP}\${APP} 제거.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

Section "바탕 화면 바로 가기" SecDesktop
  CreateShortcut "$DESKTOP\${APP}.lnk" "$INSTDIR\${EXE}"
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "KnuPdf 프로그램, 텍스트 인식(OCR) 모델, 라이선스 파일"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "바탕 화면에 KnuPdf 바로 가기를 만듭니다"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$DESKTOP\${APP}.lnk"
  RMDir /r "$SMPROGRAMS\${APP}"
  DeleteRegKey HKLM "Software\Classes\KnuPdf.Document"
  DeleteRegValue HKLM "Software\Classes\.pdf\OpenWithProgids" "KnuPdf.Document"
  DeleteRegKey HKLM "Software\Classes\Applications\${EXE}"
  DeleteRegKey HKLM "${UNINST_KEY}"
  DeleteRegKey HKLM "Software\${APP}"
  RMDir /r "$INSTDIR\models"
  Delete "$INSTDIR\${EXE}"
  Delete "$INSTDIR\KnuPdf.ico"
  Delete "$INSTDIR\LICENSE-KnuPdf.txt"
  Delete "$INSTDIR\LICENSE-MIT"
  Delete "$INSTDIR\LICENSE-APACHE"
  Delete "$INSTDIR\NOTICE"
  Delete "$INSTDIR\LICENSE-icon.txt"
  Delete "$INSTDIR\OFL-KnupdfHangul.txt"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"
SectionEnd
