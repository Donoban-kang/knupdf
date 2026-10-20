KnuPdf 0.8.2 시험판입니다. 0.8.1에서 바뀐 점만 적었습니다. 기능 전체는 [0.8.0 안내](https://github.com/Donoban-kang/knupdf/blob/main/packaging/windows-knupdf/RELEASE-NOTES-0.8.0.md)를 보세요.

## 바뀐 점

- **한글(HWPX) 변환을 두 가지 방식으로**: PDF 내보내기의 "한글 (.hwpx)"는 고쳐 쓰기 좋은 흐름 방식입니다. 새로 생긴 "한글 (.hwpx) - 원본 모양 그대로"는 PDF와 같은 배치로 글자 줄·선·상자·그림을 제자리에 놓습니다.
- **한글 변환의 내용 손상 수정**: 실제 선이 그려진 표만 표로 만들고, 위아래로 합쳐진 칸, 목록 기호(-, ※, ○, 가., 1.)로 시작하는 문단, 하이픈(-)이 그대로 남습니다.
- **인쇄 글자 선명도 개선**: Windows가 인쇄할 때 그림으로 바꿔 보내는 부분의 해상도가 프린터 설정에 따라 150dpi로 떨어지던 문제를 고쳐, 300~600dpi로 인쇄합니다.
- **기본 인쇄 용지 A4**: 인쇄 창을 처음 열면 A4가 선택되어 있습니다(이전에는 US Letter).

## 내려받기 (Windows 10/11, 64비트)

| 파일 | 용도 |
|---|---|
| `KnuPdf-0.8.2-windows-x64-setup.exe` | 설치 프로그램. `C:\Program Files\KnuPdf`에 설치하고 시작 메뉴·바탕 화면 바로 가기를 만듭니다. 이전 버전이 설치된 PC에서는 덮어서 설치하면 됩니다. |
| `KnuPdf-0.8.2-windows-x64-portable.zip` | 설치 없이 쓰는 휴대용판. 압축을 풀고 `KnuPdf.exe`를 실행합니다. |

처음 실행할 때의 SmartScreen 경고, 알려진 제한, 라이선스는 0.8.0과 같습니다. 한글 변환 결과와 인쇄 선명도는 아직 실제 한컴오피스·프린터에서 시험하지 않았습니다.

## 파일 확인값 (SHA-256)

```
SETUP_SHA  KnuPdf-0.8.2-windows-x64-setup.exe
PORTABLE_SHA  KnuPdf-0.8.2-windows-x64-portable.zip
```
