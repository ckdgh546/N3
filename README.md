# N3 Loop 8.0 — Windows + Android / ChatGPT 화면 안 즉시 해석

원하는 동작만 남긴 버전입니다.

**가사 붙여넣기 → `GPT로 바로 해석` → 같은 화면에 한국어 해석 표시**

처음 누를 때만 ChatGPT 로그인/사용 허용 화면이 한 번 열리고, 로그인이 끝나면 원래 눌렀던 해석 요청이 자동으로 이어집니다. 이후에는 로그인 화면 없이 바로 해석됩니다.

- API 키 입력 없음
- BAT 없음
- 별도 Node/로컬 서버 실행 없음
- ChatGPT 창으로 가사 보내기/복사하기 없음
- 번역 결과 다시 붙여넣기 없음
- Windows/Android 동일한 N3 Loop 코드
- 해석 완료 뒤 N3 단어·문법 자동 분석도 함께 갱신

## 기존 GitHub Pages 업데이트
기존 `ckdgh546/N3` 저장소 루트의 아래 3개만 `github-pages-update/` 안의 파일로 교체할 수 있습니다.

- `index.html`
- `sw.js`
- `player.html`

GitHub Pages 자체는 일반 브라우저용 학습 페이지이고, ChatGPT 플랜 즉시 해석은 설치형 Windows/Android 앱에서 동작합니다.

## Windows / Android 빌드
프로젝트 전체를 GitHub 저장소 루트에 Push하면 `.github/workflows/build-apps.yml`이 자동으로 빌드합니다.

GitHub → Actions → **Build N3 Loop Apps**에서 결과 파일을 받습니다.

- `N3-Loop-Windows`: Windows 설치 파일
- `N3-Loop-Android`: Android APK

로컬 PC에서 Rust나 Android Studio를 직접 설치할 필요는 없습니다.
