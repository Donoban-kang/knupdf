KnuPdf 0.8.4 시험판입니다. 0.8.3에서 바뀐 점만 적었습니다. 기능 전체는 [0.8.0 안내](https://github.com/Donoban-kang/knupdf/blob/main/packaging/windows-knupdf/RELEASE-NOTES-0.8.0.md)를 보세요.

## 바뀐 점

- **여러 쪽 PDF 변환에서 스캔 쪽이 빠지던 문제 수정**: 1쪽은 글자로 된 공문이고 2쪽부터 스캔한 붙임인 문서를 변환하면, 지금까지는 모든 형식(한글, Word, PowerPoint, HTML, RTF)에서 첫 쪽만 남았습니다. 이제 스캔 쪽도 빠짐없이 들어갑니다.
- **스캔 쪽을 고쳐 쓸 수 있는 글자로 변환(Windows 한국어 문자 인식)**: 스캔 쪽은 Windows에 들어 있는 한국어 문자 인식(OCR)으로 읽어 글자 문단으로 넣습니다. 처리는 이 PC 안에서만 하고, 문서를 밖으로 보내지 않습니다. 변환이 끝나면 "스캔 쪽 N개의 글자를 인식해 넣었습니다"라고 알려 줍니다. 인식 결과는 반드시 원본과 대조해 확인하세요. 표의 칸 구조, 도장, 손글씨는 살리지 못합니다.
- **한국어 문자 인식이 없는 PC**: 스캔 쪽을 그림으로 넣고, 알림으로 설정 방법을 안내합니다. Windows 설정 ▸ 시간 및 언어 ▸ 언어 ▸ 한국어 ▸ 언어 옵션에서 "광학 문자 인식"을 추가하면 됩니다.
- **보안 업데이트**: 암호 라이브러리(crypto-bigint, ctutils)의 보안 권고(RUSTSEC-2026-0385, -0387)를 반영했습니다.

## 내려받기 (Windows 10/11, 64비트)

| 파일 | 용도 |
|---|---|
| `KnuPdf-0.8.4-windows-x64-setup.exe` | 설치 프로그램. `C:\Program Files\KnuPdf`에 설치하고 시작 메뉴·바탕 화면 바로 가기를 만듭니다. 이전 버전이 설치된 PC에서는 덮어서 설치하면 됩니다. |
| `KnuPdf-0.8.4-windows-x64-portable.zip` | 설치 없이 쓰는 휴대용판. 압축을 풀고 `KnuPdf.exe`를 실행합니다. |

설치 파일은 코드 서명이 없어 Windows Defender가 잘못 탐지할 수 있습니다. 그럴 때는 휴대용 zip을 쓰세요. 처음 실행할 때의 SmartScreen 경고, 알려진 제한, 라이선스는 0.8.0과 같습니다.

## 파일 확인값 (SHA-256)

```
SETUP_SHA  KnuPdf-0.8.4-windows-x64-setup.exe
PORTABLE_SHA  KnuPdf-0.8.4-windows-x64-portable.zip
```
