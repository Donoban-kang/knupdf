KnuPdf 첫 시험판입니다. [PdfCraft](https://github.com/storytold/pdfcraft)(MIT 또는 Apache-2.0)를 바탕으로 화면 전체를 한국어로 바꾸고, 한글 글꼴과 전용 아이콘을 넣었습니다.

## 내려받기 (Windows 10/11, 64비트)

| 파일 | 용도 |
|---|---|
| `KnuPdf-0.8.0-windows-x64-setup.exe` | 설치 프로그램. `C:\Program Files\KnuPdf`에 설치하고 시작 메뉴·바탕 화면 바로 가기를 만듭니다. |
| `KnuPdf-0.8.0-windows-x64-portable.zip` | 설치 없이 쓰는 휴대용판. 압축을 풀고 `KnuPdf.exe`를 실행합니다. 설정은 같은 폴더에 저장됩니다. |

## 처음 실행할 때

코드 서명이 없어 Windows SmartScreen이 "Windows의 PC 보호" 경고를 띄웁니다. **추가 정보 → 실행**을 누르면 설치됩니다. 기관 PC에 배포하려면 기관 정책에 따른 소프트웨어 반입 절차를 거쳐 주세요.

## 알려진 제한

- 실제 Windows PC에서의 설치·실행은 아직 충분히 시험하지 않았습니다(시험판). 문제가 있으면 Issues에 남겨 주세요.
- 텍스트 인식(OCR) 모델은 라틴 문자 전용이라 한글 스캔본은 인식하지 못합니다.
- 파일 이름 등이 섞인 일부 엔진 오류 메시지와 운영체제 오류 문구는 영어로 나올 수 있습니다.

## 파일 확인값 (SHA-256)

```
c39dfee31e014fc4b7f6dd67f829030f35b16e14c61df938290a7b0b9a1f3453  KnuPdf-0.8.0-windows-x64-setup.exe
883d775b1b51e407032110c2c1b8255cbfe830f617c5012b82da649360c322f9  KnuPdf-0.8.0-windows-x64-portable.zip
```

## 라이선스

프로그램은 MIT 또는 Apache-2.0, KnuPdf Hangul 글꼴(나눔고딕 기반)은 SIL OFL 1.1, OCR 모델(ocrs-models)은 CC BY-SA 4.0입니다. KnuPdf는 PdfCraft나 ArtCraft 개발팀의 공식 제품이 아닙니다.
