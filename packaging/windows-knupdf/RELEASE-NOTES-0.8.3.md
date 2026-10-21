KnuPdf 0.8.3 시험판입니다. 0.8.2에서 바뀐 점만 적었습니다. 0.8.2의 변경(한글 변환 개선, 인쇄 선명도, 기본 용지 A4)은 [0.8.2 안내](https://github.com/Donoban-kang/knupdf/blob/main/packaging/windows-knupdf/RELEASE-NOTES-0.8.2.md)를 보세요.

## 바뀐 점

- **업데이트 확인을 KnuPdf 릴리스로 연결**: 도움말 ▸ 업데이트 확인이 원본 PdfCraft의 릴리스를 보고 "0.9.0이 나왔다"고 잘못 안내하던 문제를 고쳤습니다. 이제 이 페이지(KnuPdf 릴리스)의 최신 버전을 알려 주고, 내려받기를 누르면 이 페이지로 연결됩니다.
- 0.8.2 이전 버전에서는 업데이트 확인이 계속 PdfCraft 0.9.0을 안내합니다. 그 안내는 무시하고 이 페이지에서 내려받아 주세요. 0.8.3부터는 올바르게 안내합니다.

## 내려받기 (Windows 10/11, 64비트)

| 파일 | 용도 |
|---|---|
| `KnuPdf-0.8.3-windows-x64-setup.exe` | 설치 프로그램. `C:\Program Files\KnuPdf`에 설치하고 시작 메뉴·바탕 화면 바로 가기를 만듭니다. 이전 버전이 설치된 PC에서는 덮어서 설치하면 됩니다. |
| `KnuPdf-0.8.3-windows-x64-portable.zip` | 설치 없이 쓰는 휴대용판. 압축을 풀고 `KnuPdf.exe`를 실행합니다. |

설치 파일은 코드 서명이 없어 Windows Defender가 잘못 탐지할 수 있습니다(0.8.2 설치 파일에서 확인됨). 그럴 때는 휴대용 zip을 쓰세요. 처음 실행할 때의 SmartScreen 경고, 알려진 제한, 라이선스는 0.8.0과 같습니다.

## 파일 확인값 (SHA-256)

```
517c11bf147416ae21806ccdeef2828f59a883e1f8c5740a8a36e30f7fe6a8b6  KnuPdf-0.8.3-windows-x64-setup.exe
ea1fa4239c523227736eb1588374302ef74cb0ad145e7d0f3c14d71346c5f69d  KnuPdf-0.8.3-windows-x64-portable.zip
```
