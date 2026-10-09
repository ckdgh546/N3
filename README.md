# N3 Loop · GitHub Pages 배포판

이 폴더 전체를 GitHub 저장소의 최상위 경로에 올리면 정적 웹사이트로 실행됩니다.

## 가장 쉬운 배포 방법

1. GitHub에서 새 저장소를 만듭니다. 저장소 이름 예시: `n3-loop`.
2. 이 ZIP의 압축을 푼 뒤 `index.html`, `assets`, `sw.js`, `manifest.webmanifest`, `.nojekyll`을 포함한 **모든 파일과 폴더**를 저장소 루트에 올립니다.
3. 저장소의 **Settings → Pages**로 이동합니다.
4. **Build and deployment → Source**를 `Deploy from a branch`로 선택합니다.
5. Branch는 `main`, Folder는 `/(root)`를 선택하고 저장합니다.
6. 배포가 끝나면 다음 형태의 주소로 접속합니다.
   - `https://사용자이름.github.io/n3-loop/`

## 중요

- `index.html`을 컴퓨터에서 직접 더블클릭하면 YouTube 오류 153이 다시 날 수 있습니다. 반드시 GitHub Pages의 `https://` 주소로 접속합니다.
- 학습 진도·퀴즈 기록·가사 메모는 접속한 브라우저의 저장소에 보관됩니다. 다른 브라우저나 다른 기기에서는 별도 진도로 시작합니다.
- 서비스 워커가 열어 본 문제 이미지와 MP3를 브라우저 캐시에 저장합니다. 첫 이용에는 인터넷 연결이 필요합니다.
- 사이트 업데이트 후 예전 화면이 계속 나오면 브라우저 새로고침을 한 번 하거나 사이트 데이터/캐시를 지웁니다.

## 폴더 구조

```text
index.html
manifest.webmanifest
sw.js
.nojekyll
assets/
  icons/
  exams/
    2012/
    2018/
```

## 저작권·공개 범위

GitHub Pages 저장소가 공개 저장소라면 사이트에 포함한 문제 이미지와 청해 파일도 공개 접근 가능한 상태가 됩니다. 공개 배포 범위는 원자료의 이용 조건을 확인한 뒤 결정하세요.
