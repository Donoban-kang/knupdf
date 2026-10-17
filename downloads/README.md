# KnuPdf 내려받기

| 파일 | 크기 | 설명 | 내려받기 |
|---|---|---|---|
| `KnuPdf-0.8.0-windows-x64-setup.exe` | 28 MB | **설치 프로그램** (권장). 시작 메뉴·바탕 화면 바로 가기를 만들어요 | [⬇ 설치 파일 받기](https://github.com/Donoban-kang/knupdf/raw/main/downloads/KnuPdf-0.8.0-windows-x64-setup.exe) |
| `KnuPdf-0.8.0-windows-x64-portable.zip` | 36 MB | **휴대용**. 설치 없이 압축을 풀고 `KnuPdf.exe` 실행. USB에 넣어 써도 돼요 | [⬇ 휴대용 zip 받기](https://github.com/Donoban-kang/knupdf/raw/main/downloads/KnuPdf-0.8.0-windows-x64-portable.zip) |

- 지원: Windows 10 / 11 64비트 · 버전 0.8.0 (시험판) · 만든이 강돈호
- 설치 방법은 [쉬운 사용 설명서 미션 1](../docs/manual/README.md#미션-1-컴퓨터에-설치하기)을 보세요.
- 처음 실행하면 Windows SmartScreen이 "Windows의 PC 보호" 경고를 띄워요(코드 서명 전). **추가 정보 → 실행**을 누르면 설치돼요. 기관 PC는 담당 부서의 반입 절차를 따라 주세요.

## 파일 확인값 (SHA-256)

받은 파일이 바뀌지 않았는지 확인할 때 써요. PowerShell에서 `Get-FileHash 파일이름`을 실행해 아래 값과 같은지 비교해요.

```
42a3efbfab43e047ca11f86a37453060c23affddeed29adbbe3a1a0d455df621  KnuPdf-0.8.0-windows-x64-setup.exe
1fcfc401367c03274453f4b728d312de0c85f6b4f07884977cbed54641812005  KnuPdf-0.8.0-windows-x64-portable.zip
```

## 이 버전에 들어 있는 것

화면 전체 한국어, 한글 글꼴, 한국 개인정보 가리기(주민·외국인등록번호, 전화, 사업자, 계좌, 여권, 운전면허, 이메일, 카드), 페이지 정리·합치기·분할, 주석, 양식, 채우기 및 서명, 디지털 서명, 암호 보호, 압축, 텍스트 인식(OCR, 라틴 문자).
자세한 내용: [RELEASE-NOTES-0.8.0.md](../packaging/windows-knupdf/RELEASE-NOTES-0.8.0.md)

> 빌드 방법(이 파일을 직접 다시 만들기): `packaging/windows-knupdf/build-linux.sh` · 관리 안내는 [HANDOFF.md](../HANDOFF.md)
