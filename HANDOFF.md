# KnuPdf 작업 인계 (HANDOFF)

다른 PC에서, 또는 다른 사람·AI 도구가 이어서 작업할 때 처음 읽는 문서입니다.
무엇을 왜 만드는지는 [KnuPdf-PRD.md](KnuPdf-PRD.md)에 있습니다.

- 마지막 갱신: 2026-10-20 (0.8.2: 한글 변환 개선, 인쇄 선명도, 기본 용지 A4)
- 담당: 강돈호
- 저장소: https://github.com/Donoban-kang/knupdf (`main`)
- 현재 버전: 0.8.2 (0.8.0은 원본 PdfCraft 버전을 따랐고, 0.8.1부터 KnuPdf 수정 배포마다 올림)

## 1. 한눈에 보는 현재 상태

| 항목 | 상태 |
|---|---|
| 한국어 화면 | 완료. 번역 누락 0(테스트로 확인) |
| 이름·아이콘·정보 창 | 완료. KnuPdf, K 타일 아이콘, "만든이: 강원대학교 강돈호", 게시자 강원대학교 |
| Windows 설치 파일·휴대용 zip | 빌드 완료. **실제 Windows에서는 아직 미시험**(Wine에서 실행·창 제목·정보 창만 확인) |
| 내려받기 | `downloads/`에 설치 파일·zip을 커밋해 README에서 바로 받게 함(임시). GitHub Releases는 **미등록**(6절). Releases에 올린 뒤에는 `downloads/`의 파일을 지우고 링크를 Releases로 바꾸는 것을 권장 |
| 웹판 | claude.ai Artifact로 비공개 게시: https://claude.ai/artifact/CzyCoaMnWXithtxtsuF2hq |
| 한국 개인정보 가리기 | 완료. PDF 가리기 → 텍스트 찾아 가리기 → 패턴 → "한국 개인정보 전체"(기본값) 또는 항목별 |
| 한글(HWPX) 내보내기 | 완료. PDF 내보내기 → 내보낼 형식 → "한글 (.hwpx)"(고쳐 쓰기 좋은 흐름 방식) 또는 "한글 (.hwpx) - 원본 모양 그대로"(PDF와 같은 배치). **실제 한컴오피스 한글에서는 아직 미시험**(구조 검사 도구와 자체 미리보기로만 확인) |
| 다음 할 일 | 9절 |

## 2. 저장소 구조와 기록

- 이 저장소의 첫 KnuPdf 커밋 `f9142d3`은 원본 PdfCraft 소스를 그대로 가져온 것입니다.
  - 원본 커밋: `storytold/pdfcraft@924c4da4ac2879ac9107c6c82e20ac6be5570f0a`
  - **원본의 커밋 기록은 들어 있지 않습니다.** 원본 갱신은 5절의 패치 방식으로 받습니다.
- 그 위의 커밋들이 KnuPdf 변경분입니다: `git log --oneline f9142d3..main`

### KnuPdf에서 바꾸거나 더한 파일

| 위치 | 내용 |
|---|---|
| `crates/ui-egui/src/i18n/ko.tsv` | 한국어 번역 사전(형식은 같은 폴더 `ja.tsv` 머리말 참고) |
| `crates/ui-egui/src/i18n/mod.rs` | `ko` 언어 등록. `fmt()`가 `{e}`, `{error}` 같은 오류 값도 번역 사전에서 찾음. 테스트 `korean_covers_everything`, `korean_fmt_translates_inserted_errors_only` |
| `crates/ui-egui/src/lib.rs` | 기본 언어 `ko`, 창 제목 "… — KnuPdf", `notify_error` 번역 |
| `crates/ui-egui/src/theme.rs` | 한글 글꼴(KnupdfHangul) 등록 |
| `crates/ui-egui/src/home.rs`, `chrome.rs`, `widgets.rs`, `dialogs.rs` | ArtCraft 로고·Discord 제거, 정보 창(버전, 만든이) |
| `crates/redact/src/patterns.rs` | 한국 개인정보 패턴(`KrAll`, `KrResident`, `KrPhone`, `KrBusiness`, `KrAccount`, `KrPassport`, `KrDriver`)과 테스트. 자동화 도구 `redact_mark`의 `pattern` 값은 `kr-all`, `kr-resident` … |
| `crates/export/src/hwpx.rs` | 한글(HWPX, OWPML) 내보내기 작성기와 테스트. 엔진 `compare.rs`의 `OfficeFormat::Hwpx`, 명령 `export.hwpx`, 도구 목록 항목, 자동화 도구 `doc_export_office`(경로 확장자 `.hwpx`)에 연결 |
| `crates/engine/src/commands.rs`, `links.rs` | 도움말 메뉴에서 Discord·ArtCraft 링크 숨김, "KnuPdf 정보" |
| `assets/fonts/KnupdfHangul.ttf`, `OFL-KnupdfHangul.txt` | 나눔고딕 한글 부분, 이름 변경(OFL 조건) |
| `assets/fonts/NanumBarunGothic.ttf`, `NanumBarunGothicBold.ttf`, `OFL-NanumBarunGothic.txt` | 나눔바른고딕 원본(수정 없음, OFL). PDF에 내장되지 않은 한국어 글꼴(Adobe-Korea1)을 대신 그리는 글꼴. 한글·영문·KS X 1001 한자 포함. 원본 규칙(AGENTS.md §1.4, 글꼴은 craft-fonts에)의 KnuPdf 예외 |
| `crates/fonts/src/craft.rs`, `fallback.rs` | `KOREAN_FONTS`, `korean_document_font()`. 대체 정책에서 Korea1을 "대체 글꼴 없음"에서 나눔바른고딕으로 변경 |
| `crates/render/src/raster.rs` | `korean_fallback()`: 내장되지 않은 한국어 CID 글꼴을 나눔바른고딕(굵게는 Bold)으로 그림. 테스트 `non_embedded_korean_cid_fonts_draw_hangul_latin_and_hanja` |
| `vendor/hayro-interpret/src/font/cid.rs` | 유니코드 CMap(`UniKS-UCS2-H` 등)에 없는 글자 코드를 1바이트가 아니라 글자 단위(2바이트, UTF-16 짝은 4바이트)로 읽고, 그 글자로 그림·추출. 가운뎃점 `·`(U+00B7)이 '뜀'으로 바뀌던 문제. 테스트 `unmapped_codes_in_unicode_cmaps_are_read_whole` |
| `assets/app-icon-knupdf/` | 아이콘 원본(SVG)과 PNG·ICO, 라이선스 |
| `apps/pdfcraft/build.rs`, `src/main.rs` | exe 아이콘·제품 정보(KnuPdf, 회사명 강원대학교), 창 아이콘 |
| `apps/pdfcraft-web/index.html` | 웹 탭 제목·아이콘, `lang="ko"` |
| `packaging/windows-knupdf/` | NSIS 설치 스크립트, 라이선스 안내문, Linux용 빌드 스크립트 |
| `packaging/web-artifact/` | 웹판을 Artifact로 올리기 위한 로더 페이지와 패키징 스크립트 |
| `README.md`, `KnuPdf-PRD.md`, `HANDOFF.md` | 문서 |
| `docs/manual/` | 쉬운 사용 설명서(`README.md`는 GitHub용, `index.html`은 꾸민 페이지, `images/`는 번호 표시한 화면). 화면이 바뀌면 그림도 다시 찍어야 함 |

실행 파일 이름(`pdfcraft.exe`), crate 이름(`pdfcraft-*`), 설정 폴더 이름(`%APPDATA%\PdfCraft`, `PdfCraftData`)은
원본 갱신을 받기 쉽도록 **일부러 그대로** 두었습니다. 설치할 때 exe만 `KnuPdf.exe`로 바꿔 넣습니다.

## 3. 개발 환경 준비

### 공통
- Rust 1.95 이상: https://rustup.rs
- `git clone https://github.com/Donoban-kang/knupdf && cd knupdf`
- 데스크톱 실행: `cargo run -p pdfcraft --release -- 파일.pdf`
- 빌드 속도: 처음 빌드는 10분 안팎 걸립니다. 원본 안내대로 `-j 2`~`-j 4`로 제한하는 것이 안전합니다.

### Linux(Ubuntu)에서 Windows판까지 만들기 — 이번 작업에서 쓴 방식
```sh
sudo apt-get install -y gcc-mingw-w64-x86-64 g++-mingw-w64-x86-64 nsis
rustup target add x86_64-pc-windows-gnu
packaging/windows-knupdf/build-linux.sh        # → dist/windows/ 에 setup.exe, portable.zip
```

### Windows PC에서 직접 만들기
1. Visual Studio Build Tools(C++), Rust, NSIS(https://nsis.sourceforge.io)를 설치합니다.
2. `cargo build --release -p pdfcraft` → `target\release\pdfcraft.exe`
3. 스테이징 폴더를 만듭니다. 들어갈 파일은 `build-linux.sh`와 같습니다.
   - `KnuPdf.exe`(위 exe 이름 변경), `KnuPdf.ico`, 라이선스 파일들, `models\`(OCR 모델 2개와 `ATTRIBUTION.txt`)
4. 다음 명령으로 설치 파일을 만듭니다.
   `makensis -DVERSION=0.8.2 -DSTAGE=<스테이징 폴더> -DOUT=KnuPdf-0.8.2-windows-x64-setup.exe packaging\windows-knupdf\knupdf.nsi`
- WSL(Ubuntu)이 있으면 위 Linux 방식을 그대로 써도 됩니다.

### 웹판
```sh
cargo install trunk --locked          # 또는 trunk 릴리스 바이너리
rustup target add wasm32-unknown-unknown
(cd apps/pdfcraft-web && trunk build --release)   # → dist/web
packaging/web-artifact/pack.sh                     # → dist/web-artifact (Artifact용 분할본)
```
- `dist/web`은 일반 웹 서버(기관 서버 등)에 그대로 올리면 됩니다.
- claude.ai Artifact는 파일 하나당 약 15MB 제한이 있습니다. 그래서 엔진(약 40MB)을 압축해 두 조각으로 나눠 올립니다.
- Artifact를 갱신할 때는 Claude에게 `dist/web-artifact/knupdf.html`을 위 주소에 다시 게시해 달라고 하면 됩니다. 함께 올릴 파일은 `pdfcraft.js`, `part0.wasm`, `part1.wasm`, `knupdf.svg`입니다.

## 4. 번역 작업 방법

- 화면 문구는 코드에서 `tl!("영어 문구")`로 씁니다. 번역은 `ko.tsv`에 `<빈 칸>\t영어\t한국어` 한 줄로 넣습니다.
- `{name}` 같은 자리표시는 양쪽에 똑같이 있어야 하고, 끝의 "…"도 맞춰야 합니다. 어기면 그 줄이 무시됩니다.
- 누락 검사:
  ```sh
  KO_MISSING=/tmp/ko_missing.tsv cargo test -p pdfcraft-ui-egui --lib -- korean_covers_everything bundled_catalogs_are_consistent
  ```
  실패하면 `/tmp/ko_missing.tsv`에 빠진 문구 목록이 생깁니다.
- 엔진의 고정 오류 문구도 `ko.tsv`에 그대로 넣으면 화면에서 번역됩니다(`fmt()`의 `{e}` 조회).

### 알려진 테스트 실패(예상된 것)
- 다른 언어(일본어, 독일어 등)의 `*_covers_*` 테스트가 실패합니다. KnuPdf에서 바꾼 영어 문구가 그 언어 사전에는 없기 때문입니다.
- `date_languages_are_the_interface_languages`가 실패합니다. 엔진 날짜 모듈에 한국어 규칙이 없어서입니다(PRD F-04).
- 원본 CI(`.github/workflows`)는 원본 기준이라 KnuPdf 저장소에서는 일부 단계가 실패할 수 있습니다. 정리가 필요합니다(9절).

## 5. 원본(PdfCraft) 갱신 받아오기

기록이 이어져 있지 않으므로 "기준 커밋과 새 커밋의 차이"를 패치로 적용합니다.
```sh
git remote add upstream https://github.com/storytold/pdfcraft
git fetch upstream main
BASE=924c4da4ac2879ac9107c6c82e20ac6be5570f0a          # 마지막으로 반영한 원본 커밋
git switch -c sync-upstream
git diff $BASE upstream/main -- . ':(exclude)docs/brand' ':(exclude)README.md' > /tmp/upstream.patch
git apply -3 /tmp/upstream.patch         # 충돌은 <<< >>> 표시를 보고 정리
cargo test -p pdfcraft-ui-egui --lib -- korean_covers_everything   # 새 영어 문구 번역 추가
git commit -am "Sync PdfCraft upstream to <새 원본 커밋>"
```
- 반영 후 이 문서의 `BASE` 값을 새 원본 커밋으로 바꿔 둡니다.
- 원본이 ArtCraft 로고·링크를 다시 추가했는지 확인합니다(`grep -rn artcraft crates apps`).

## 6. GitHub Releases에 설치 파일 올리기 (수동)

작업한 클라우드 세션에서는 릴리스 생성이 막혀 있어 직접 올려야 합니다.
1. https://github.com/Donoban-kang/knupdf/releases/new 를 엽니다.
2. 태그 `v0.8.2`을 만들고, 대상은 `main`으로 둡니다. 제목은 `KnuPdf 0.8.2 (시험판)`으로 합니다.
3. 설명은 `packaging/windows-knupdf/RELEASE-NOTES-0.8.2.md` 내용을 붙여 넣습니다. 파일 확인값(SHA-256)은 새로 빌드한 파일 기준으로 고칩니다. 릴리스 페이지에서는 상대 주소 링크가 열리지 않으므로, 노트 안의 링크는 `https://github.com/Donoban-kang/knupdf/blob/main/...` 같은 전체 주소로 씁니다.
4. `KnuPdf-0.8.2-windows-x64-setup.exe`와 `...-portable.zip`을 첨부합니다.
5. Set as a pre-release에 체크한 뒤 Publish release를 누릅니다.

## 7. 점검 방법

- **실제 Windows(우선)**: 설치·실행·제거와 PDF 열기·저장·인쇄·합치기·가리기·암호를 점검합니다(PRD F-01).
- **Linux에서 Wine으로 빠르게 확인하는 방법(참고용)**
  - 준비: `apt install wine64 xvfb xdotool`
  - Wine에는 Windows 10의 `windows.data.pdf.dll`이 없어 바로 실행되지 않습니다. 그 함수 하나(`PdfCreateRenderer`)만 흉내 내는 빈 DLL을 mingw로 만들어 exe 옆에 두면 실행됩니다. **이 DLL은 시험용이며 절대 배포물에 넣지 않습니다.**
  - 실행: `xvfb-run wine64 KnuPdf.exe` 뒤 `xwd`로 화면을 캡처하고, `xdotool`로 클릭해 메뉴와 정보 창을 확인했습니다.

## 8. 라이선스·상표 주의

- ArtCraft 이름·로고는 오픈 소스가 아니며, 수정판에서는 빼야 합니다(원본 `docs/brand/LICENSE-brand.txt`, 이 저장소에서는 삭제).
- 나눔고딕을 잘라 쓴 글꼴은 OFL 조건상 원래 이름을 쓸 수 없습니다. 그래서 "KnuPdf Hangul"로 바꿨습니다.
- 배포물에는 `LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`, `OFL-KnupdfHangul.txt`, 아이콘·모델 출처 파일을 넣습니다(설치 스크립트가 처리).
- 저장소가 공개 상태이므로 기관 문서, 개인정보, 인증서·암호를 커밋하지 않습니다.

## 9. 다음 할 일 (우선순위 순)

1. **실제 Windows 점검**(PRD F-01). 문제가 있으면 Issues에 기록합니다.
   - **인쇄 선명도**: 글자가 흐리게 인쇄된다는 보고가 있었습니다. Direct2D가 그림으로 바꿔 그리는 부분의 해상도를 300~600dpi로 고정했습니다(`vendor/winprint`, `raster_dpi`). 실제 프린터에서 확인되지 않았습니다. 여전히 흐리면 프린터 드라이버의 인쇄 품질(초안·절약 모드)과 "이미지로 인쇄 600dpi"를 비교해 봅니다.
2. **GitHub Releases 등록**(6절).
3. **한국 개인정보 가리기 보완**(F-02 후속): 실제 기관 문서로 오탐·누락 점검, 필요한 패턴(건강보험증 번호 등) 추가.
   - 코드: `crates/redact/src/patterns.rs`. 테스트: `cargo test -p pdfcraft-redact --lib patterns`
   - 명령줄로 확인: `pdfcraft-cli run --script steps.json` (doc_open → redact_mark `pattern: kr-all` → redact_apply → doc_save)
4. **CI 정리**: 원본 GitHub Actions 중 KnuPdf에 필요 없는 것(macOS·FreeBSD·서명 등)을 끄고, Windows 설치 파일을 자동으로 만드는 작업을 추가합니다.
5. **한글(HWPX) 내보내기 점검**(F-11): 한컴오피스 한글 2018 이상에서 열기·표·그림·쪽 나눔 확인. 쪽마다 크기가 다른 PDF는 첫 쪽 크기로 통일되는 한계 개선(구역 나누기)
   - 변환 방식이 두 가지입니다.
     - **흐름 방식**(`export.hwpx`): 고쳐 쓰기 좋은 문서입니다. 괘선 표는 선을 읽어 병합 셀·행 높이·배경색을 살리고, 본문 줄은 목록 기호(-, ※, ○, 가., 1. 등)에서 문단을 나눕니다. 코드는 `crates/export/src/ruled.rs`와 `hwpx.rs`의 `Section::table`입니다.
     - **원본 모양 그대로**(`export.hwpx_layout`, 자동화 `doc_export_office` `layout: "keep"`): 줄마다 글상자, 선·칠한 사각형·그림을 종이 기준 절대 위치에 놓습니다(`hwpx.rs`의 `Layout`). 선은 `crates/render/src/vectors.rs`에서 모읍니다.
   - 두 방식 모두 한컴오피스 실물 확인이 아직 남았습니다. 특히 원본 모양 방식은 글상자 안 글자 폭(글꼴 차이)을 봐야 합니다.
6. **직인 찍기**(F-03), **한국어 날짜 형식**(F-04).
7. **화면 개편**(F-06): 강원대 블루 리본형 UI 시안을 기준으로 합니다.
8. **코드 서명**(F-07), **한글 OCR 조사**(F-05).

## 10. 이번 작업에서 만든 산출물

| 산출물 | 위치 |
|---|---|
| 소스 | 이 저장소 `main` |
| Windows 설치 파일·휴대용 zip | `build-linux.sh`로 다시 만들 수 있음(빌드 결과는 저장소에 넣지 않음) |
| 웹판 | https://claude.ai/artifact/CzyCoaMnWXithtxtsuF2hq (비공개, 소유자만 열람) |
| 화면 디자인 시안 | claude.ai Artifact "KNUPDF 화면 디자인 (강원대 블루)", "KNUPDF UI 개편 — 시작·작업 화면" |
