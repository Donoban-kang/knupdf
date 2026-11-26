KnuPdf 0.8.7 시험판입니다. 0.8.6에서 바뀐 점만 적었습니다. 기능 전체는 [0.8.0 안내](https://github.com/Donoban-kang/knupdf/blob/main/packaging/windows-knupdf/RELEASE-NOTES-0.8.0.md)를 보세요.

## 바뀐 점

- **인쇄 중 프로그램이 닫히던 문제 수정**: 사무실 복합기(PCL 6 네트워크 프린터)로 인쇄하면 "용지 2장 중 1장을 보내는 중 0%"에서 멈춘 뒤 프로그램이 닫혔습니다. 0.8.2부터 글자를 선명하게 인쇄하려고, 프린터 해상도(최대 600dpi)로 인쇄용 그림을 만들었습니다. 그래서 A4 한 장마다 약 3,500만 화소의 그림이 필요했고, 메모리가 부족해진 것으로 보입니다. 이제 300dpi로 고정했습니다. 메모리는 600dpi의 4분의 1이고, 글자는 0.8.1(150dpi)보다 선명합니다.
  - 확인한 범위: Windows용 빌드와 코드 검사까지 했습니다. 실제 프린터에서는 아직 확인하지 못했습니다. 아주 작은 글자(7pt 이하)는 600dpi보다 조금 덜 날카로울 수 있습니다.
- **보안 업데이트**: 주소(URL) 처리 라이브러리를 2.5.9로 올렸습니다(RUSTSEC-2026-0592).

## 내려받기 (Windows 10/11, 64비트)

| 파일 | 용도 |
|---|---|
| `KnuPdf-0.8.7-windows-x64-setup.exe` | 설치 프로그램. 이전 버전이 설치된 PC에서는 덮어서 설치하면 됩니다. |
| `KnuPdf-0.8.7-windows-x64-portable.zip` | 설치 없이 쓰는 휴대용판. 압축을 풀고 `KnuPdf.exe`를 실행합니다. |

설치 파일은 코드 서명이 없어 Windows Defender가 잘못 탐지할 수 있습니다. 그럴 때는 휴대용 zip을 쓰세요.

## 파일 확인값 (SHA-256)

```
3c84af257e256f5d908d1cdea8ac0afff3603e75a011c2253976ffa3f7559c69  KnuPdf-0.8.7-windows-x64-setup.exe
fe14461cadd8004bf7e40736dc1de3100d6fa21851fa23ca331ac36d40fc5561  KnuPdf-0.8.7-windows-x64-portable.zip
```
