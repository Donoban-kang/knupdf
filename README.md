# KnuPdf

한국어로 쓰는 PDF 작업 도구입니다. 오픈 소스 PDF 응용 프로그램
[PdfCraft](https://github.com/storytold/pdfcraft)(MIT 또는 Apache-2.0)를 바탕으로,
화면 전체 한국어 번역·한글 글꼴·전용 아이콘·Windows 설치 프로그램을 더했습니다.

KnuPdf는 PdfCraft나 ArtCraft 개발팀의 공식 제품이 아니며, ArtCraft 상표(이름·로고)는
이 저장소에 포함하지 않습니다.

## 내려받기 (Windows 10/11 64비트)

| | |
|---|---|
| **[⬇ 설치 파일 받기 (KnuPdf-0.8.3-windows-x64-setup.exe, 32 MB)](https://github.com/Donoban-kang/knupdf/raw/main/downloads/KnuPdf-0.8.3-windows-x64-setup.exe)** | 권장. 설치 후 시작 메뉴에서 실행 |
| [⬇ 휴대용 zip 받기 (KnuPdf-0.8.3-windows-x64-portable.zip, 40 MB)](https://github.com/Donoban-kang/knupdf/raw/main/downloads/KnuPdf-0.8.3-windows-x64-portable.zip) | 설치 없이 압축 풀고 `KnuPdf.exe` 실행 |

파일 확인값과 설치 안내: [downloads/](downloads/README.md) · 처음 실행 시 SmartScreen 경고가 나오면 **추가 정보 → 실행**

## 사용 설명서

처음 쓰는 분(초등학생도)을 위한 그림 설명서: [docs/manual](docs/manual/README.md)

## 주요 기능

PDF 읽기, 페이지 구성(회전·삭제·추출·분할·합치기), 주석, 양식 작성과 준비, 채우기 및 서명,
디지털 서명, 암호 보호, 가리기(개인정보 영구 삭제), 내보내기(Word·한글 HWPX·PowerPoint·HTML·RTF), 압축·최적화, 텍스트 인식(OCR, 라틴 문자),
접근성 검사 등 PdfCraft의 기능을 그대로 씁니다.

## PdfCraft와 달라진 점

| 항목 | 내용 | 위치 |
|---|---|---|
| 한국어 화면 | 번역 문구 약 2,900개, 기본 언어 한국어, 엔진 오류 메시지 번역 | `crates/ui-egui/src/i18n/ko.tsv` |
| 한글 글꼴 | 나눔고딕 한글 부분을 이름을 바꿔 포함 (KnuPdf Hangul, OFL 1.1) | `assets/fonts/KnupdfHangul.ttf` |
| 이름·상표 | 화면 이름 KnuPdf, ArtCraft 로고·Discord 안내 제거 | `crates/ui-egui`, `apps/` |
| 아이콘 | 강원대 블루 타일에 K | `assets/app-icon-knupdf/` |
| 한글(HWPX) 내보내기 | PDF를 한글 문서(.hwpx)로 내보내기: 문단·제목·표·그림 | `crates/export/src/hwpx.rs` |
| 한국 개인정보 가리기 | 주민등록번호·전화·사업자·계좌·여권·운전면허 패턴 | `crates/redact/src/patterns.rs` |
| Windows 설치 | 한국어 NSIS 설치 프로그램 | `packaging/windows-knupdf/` |

## 빌드

Rust(1.95 이상)가 필요합니다.

```sh
# 데스크톱 앱 실행
cargo run -p pdfcraft --release

# 웹판 (apps/pdfcraft-web에서, trunk 필요) → dist/web
trunk build --release

# Windows 64비트 (Linux에서 교차 빌드: mingw-w64 필요)
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release -p pdfcraft --target x86_64-pc-windows-gnu
# 설치 프로그램 (NSIS): packaging/windows-knupdf/knupdf.nsi 맨 위 주석 참고
```

## 라이선스

- 프로그램: MIT 또는 Apache-2.0 (`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`)
- KnuPdf Hangul 글꼴: SIL Open Font License 1.1 (`assets/fonts/OFL-KnupdfHangul.txt`)
- 그 밖의 아이콘·글꼴·모델의 출처: `ATTRIBUTION.toml`
