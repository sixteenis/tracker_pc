# 핀플 PC 앱 (1차 MVP) — Rust 구현

윈도우 PC 에서 근로자의 PC 사용/미사용을 감지하고 "근무시간 소명"을 입력/제출
받는 윈도우용 데스크톱 앱입니다. **출근/퇴근 체크는 본 앱에서 절대 수행하지
않습니다 — 핀플 스마트폰 근로자 앱에서만 처리됩니다.**

> 본 저장소는 기획서(2026-05) 의 1차 MVP 범위만 다룹니다.
> 관리자 웹 대시보드, 엑셀 다운로드, AI 분석 등은 2차 범위입니다.

---

## 1. 개발/실행 방법

### 1-1. 사전 요구사항

| 항목 | 버전 |
|------|------|
| Rust toolchain | 1.75 이상 (현재 검증: 1.95) |
| OS | 운영: Windows 10/11, 개발/검증: macOS / Linux 가능 (PC idle 감지는 stub) |

```bash
# 의존성 다운로드 + 컴파일
cargo build

# Mock API 모드로 실행 (기본 설정)
cargo run

# 단위 테스트
cargo test

# 릴리즈 빌드
cargo build --release
```

기본 빌드는 Mock API 모드로 동작하므로 핀플 서버가 없어도 로그인/정책/이벤트
전송 흐름을 끝까지 시연할 수 있습니다.

### 1-2. 설정

| 우선순위 | 위치 |
|---------|------|
| 1 (낮음) | 컴파일에 포함된 `config/default.toml` |
| 2 | OS 사용자 설정 디렉토리의 `config.toml` (예: `%APPDATA%\Pinple\PCAgent\config.toml`) |
| 3 (높음) | 환경변수 `PINPLE_API_BASE_URL`, `PINPLE_MOCK_MODE` |

실서버 연결 예:

```bash
PINPLE_API_BASE_URL=https://api.pinple.io PINPLE_MOCK_MODE=false cargo run --release
```

### 1-3. 윈도우 설치 파일 생성

1차 MVP 에서는 `cargo build --release` 산출물(`target/release/pinple_pc_agent.exe`)을
[Inno Setup](https://jrsoftware.org/isinfo.php) 또는 WiX Toolset 에 묶는 방식을
권장합니다. 다음 항목을 인스톨러 스크립트가 처리합니다.

- 설치 경로: `C:\Program Files\Pinple\PCAgent\`
- 바탕화면 바로가기 / 시작메뉴 등록
- HKCU 또는 HKLM `\Software\Microsoft\Windows\CurrentVersion\Run` 에 자동 실행 등록
- 트레이 아이콘용 `.ico` (`resources/icon.ico` 자리 마련됨)

샘플 Inno Setup 스크립트:

```iss
[Setup]
AppName=핀플 PC
AppVersion=0.1.0
DefaultDirName={pf}\Pinple\PCAgent
DefaultGroupName=핀플 PC
OutputBaseFilename=PinplePCAgent_Setup_0_1_0
Compression=lzma
SolidCompression=yes

[Files]
Source: "target\release\pinple_pc_agent.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "resources\icon.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\핀플 PC"; Filename: "{app}\pinple_pc_agent.exe"
Name: "{commondesktop}\핀플 PC"; Filename: "{app}\pinple_pc_agent.exe"

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; \
     ValueType: string; ValueName: "PinplePCAgent"; \
     ValueData: """{app}\pinple_pc_agent.exe"""; Flags: uninsdeletevalue

[Run]
Filename: "{app}\pinple_pc_agent.exe"; Description: "핀플 PC 시작"; Flags: nowait postinstall skipifsilent
```

---

## 2. 프로젝트 구조

```
pinple_pc_agent/
├── Cargo.toml                     # 의존성 / 빌드 설정
├── config/default.toml            # 기본 설정 (인터벌, 정책 fallback 등)
├── migrations/0001_init.sql       # SQLite 스키마 5개 테이블
├── docs/
│   ├── api.md                     # 9개 엔드포인트 요청/응답 예시
│   └── test_scenarios.md          # 1차 MVP 테스트 시나리오
└── src/
    ├── main.rs                    # 진입점: 설정→DB→런타임→감지/동기화→UI
    ├── app.rs                     # 공유 AppState (RwLock + Arc)
    ├── config.rs                  # AppConfig 로더
    ├── api/
    │   ├── mod.rs                 # ApiClient trait
    │   ├── client.rs              # 실 HTTP (reqwest)
    │   ├── mock.rs                # 개발용 Mock
    │   ├── endpoints.rs
    │   └── types.rs               # 9개 엔드포인트 모든 DTO
    ├── auth/
    │   ├── mod.rs                 # 로그인/자동로그인/로그아웃
    │   └── token_store.rs         # OS Credential Store (DPAPI/Keychain)
    ├── db/
    │   ├── mod.rs
    │   ├── auth_repo.rs           # 1. auth
    │   ├── events_repo.rs         # 2. local_events (PENDING/SUCCESS/FAILED)
    │   ├── idle_segments_repo.rs  # 3. idle_segments
    │   ├── explanations_repo.rs   # 4. explanations
    │   └── settings_repo.rs       # 5. settings (k/v)
    ├── monitor/
    │   ├── mod.rs                 # 백그라운드 task spawn
    │   ├── input.rs               # GetLastInputInfo (Windows-only)
    │   ├── idle_detector.rs       # 5초 polling → idle segment 생성
    │   ├── session_events.rs      # PC 잠금/잠금해제 (stub + 호출 인터페이스)
    │   ├── lifecycle.rs           # APP_STARTED / NO_PC_RECORD 자동 생성
    │   └── policy.rs              # employee→team→company→default 우선순위
    ├── sync/
    │   ├── mod.rs
    │   ├── heartbeat.rs           # 3분 주기
    │   ├── event_sync.rs          # 1분 주기 배치
    │   ├── policy_sync.rs         # 30분 주기
    │   ├── update_check.rs        # 12시간 주기
    │   └── attendance_sync.rs     # 5분 주기 출근 상태 폴링
    ├── ui/
    │   ├── mod.rs                 # egui 라우터 + 한글 폰트 자동 등록
    │   ├── login_view.rs
    │   ├── status_view.rs
    │   ├── explanation_list_view.rs
    │   ├── explanation_input_view.rs
    │   ├── settings_view.rs
    │   ├── update_view.rs
    │   └── disabled_view.rs
    ├── notify/mod.rs              # OS 토스트 (notify-rust)
    ├── device/mod.rs              # device_id (UUID, 영구) + device_name
    ├── lunch/mod.rs               # 점심 윈도우 분류
    └── util/mod.rs                # 시간 포맷
```

---

## 3. 주요 클래스 / 모듈 설명

| 모듈 | 핵심 타입/함수 | 역할 |
|------|----------------|------|
| `app::AppState` | `config / db / device / api / runtime / session(RwLock) / status(RwLock) / policy(RwLock)` | 모든 백그라운드 task 와 UI 가 공유하는 단일 상태 컨테이너 |
| `api::ApiClient` (trait) | `login / refresh / get_policy / send_heartbeat / send_events / list_explanations / submit_explanation / get_attendance / update_check` | 9개 엔드포인트 추상화. `mock_mode` 에 따라 `MockClient` / `HttpApiClient` 주입 |
| `auth::Session` | `access_token / refresh_token / company_id / employee_id / subscription` | 메모리에만 보관. DB 의 `auth` 테이블에는 비-비밀 식별값만 |
| `auth::token_store` | `save / load / clear_refresh_token` | `keyring` (Windows: DPAPI 기반 Credential Manager) |
| `monitor::idle_detector` | `run(state)` | 5초마다 `GetLastInputInfo` 호출 → threshold 초과 시 `idle_segments` 한 row 추가 + `IDLE_STARTED` 이벤트 enqueue |
| `monitor::policy::resolve` | `(policy, default) -> (seconds, scope)` | employee → team → company → default 우선순위 계산 |
| `sync::heartbeat` | `run(state)` | 3분 주기. `can_track_time = false` 면 skip |
| `sync::event_sync` | `run(state)` | 1분 주기 배치(최대 50건). 실패 시 `FAILED` 후 재시도 |
| `db::events_repo` | `enqueue / pending_batch / mark_success / mark_failed` | UUID `event_id` 기반 멱등 큐 |
| `db::idle_segments_repo` | `insert / close / list_pending_for_employee / mark_submitted` | 자리비움 구간 CRUD. `applied_idle_threshold_seconds` + `policy_scope` 함께 보관 |
| `lunch::classify` | `(start, end, policy) -> LunchClassification` | 점심 윈도우 안/밖 + 인정 시간 초과 분리 |
| `notify::show_explanation_request` | OS 토스트 표시 | 우측 하단 팝업 (Windows: WinRT toast / macOS: NSUserNotification) |
| `ui::PinpleApp` | egui `App` 구현 | 9개 화면 라우팅. 한글 폰트 자동 등록 |

---

## 4. 설정값 설명 (`config/default.toml`)

| 키 | 기본값 | 설명 |
|----|--------|------|
| `api.base_url` | `https://api.pinple.io` | 실서버 URL |
| `api.timeout_seconds` | 15 | reqwest 타임아웃 |
| `api.mock_mode` | true | Mock 클라이언트 사용 여부 |
| `app.app_version` | 0.1.0 | UI 표시 + heartbeat / update-check 전송값 |
| `intervals.idle_check_interval_seconds` | 5 | idle 폴링 주기 |
| `intervals.heartbeat_interval_seconds` | 180 | heartbeat 기본 주기 (서버 응답으로 동적 조정) |
| `intervals.event_batch_interval_seconds` | 60 | 이벤트 배치 주기 |
| `intervals.policy_check_interval_seconds` | 1800 | 정책 재조회 주기 |
| `intervals.update_check_interval_seconds` | 43200 | 업데이트 확인 주기 (12시간) |
| `intervals.max_events_per_batch` | 50 | 한 배치 최대 이벤트 수 |
| `policy_defaults.default_idle_threshold_seconds` | 600 | 서버 정책 미수신 시 fallback (10분) |
| `policy_defaults.default_lunch_start_time` | "11:30" | 점심 윈도우 시작 |
| `policy_defaults.default_lunch_end_time` | "14:00" | 점심 윈도우 종료 |
| `policy_defaults.default_lunch_allowed_minutes` | 60 | 점심 인정시간 |
| `logging.level` | info | trace/debug/info/warn/error |

---

## 5. 보안 / 개인정보 원칙 (기획서 §19, §23)

| 항목 | 처리 |
|------|------|
| 비밀번호 | **메모리에서만 사용** — `auth::login` 호출 후 즉시 drop. DB/파일/로그 어디에도 저장하지 않음 |
| `access_token` | 메모리(`auth::Session`) 에만 보관 |
| `refresh_token` | OS Credential Store (Windows: DPAPI 암호화 Credential Manager / macOS: Keychain) |
| 키보드 입력 내용 | 수집/저장/전송 ❌ |
| 화면 캡처 | ❌ |
| 방문 웹사이트 | ❌ |
| 실행 프로그램 목록 | ❌ |
| 문서명 / 메신저 내용 | ❌ |
| 키/마우스 움직임 자체 | 서버 전송 ❌. 로컬에서 idle 여부만 판단 |
| 서버 전송 이벤트 | `APP_STARTED / APP_STOPPED / IDLE_STARTED / IDLE_ENDED / PC_LOCKED / PC_UNLOCKED / PC_SHUTDOWN_DETECTED / NO_PC_RECORD / HEARTBEAT / EXPLANATION_SUBMITTED / SYNC_*` 만 |

---

## 6. CI / Release (GitHub Actions)

`.github/workflows/` 에 두 워크플로우가 있습니다.

| 파일 | 트리거 | 동작 |
|------|--------|------|
| `ci.yml` | `main` push, PR | Linux/Windows/macOS 3개 OS 에서 `cargo test` + `cargo build` |
| `release.yml` | `v*` 태그 push, 또는 수동 실행 | Windows x64 / macOS Intel / macOS Apple Silicon 빌드 → GitHub Release 자동 첨부. `installer/pinple.iss` 가 있으면 Inno Setup 인스톨러 (`PinplePCAgent_Setup_<버전>.exe`) 도 함께 생성 |

### 6-1. 새 릴리즈 만드는 법

```bash
# 버전 올리고 커밋
sed -i '' 's/^version = .*/version = "0.2.0"/' Cargo.toml   # macOS BSD sed
git add Cargo.toml
git commit -m "release: v0.2.0"

# 태그 푸시 → 워크플로우 자동 실행
git tag v0.2.0
git push origin main --tags
```

GitHub Actions 의 `Release Build` 워크플로우가 끝나면 `Releases` 탭에 다음 산출물이 첨부됩니다:

- `pinple_pc_agent-x86_64-pc-windows-msvc.zip`
- `PinplePCAgent_Setup_v0.2.0.exe` (Inno Setup 인스톨러)
- `pinple_pc_agent-x86_64-apple-darwin.tar.gz`
- `pinple_pc_agent-aarch64-apple-darwin.tar.gz`

### 6-2. 코드 서명 (2차에서 결정)

| OS | 미서명 시 사용자 경험 | 서명 방법 |
|----|----------------------|----------|
| Windows | SmartScreen 경고 → "추가 정보" → "실행" | EV 코드 사이닝 인증서 구입 후 `signtool.exe` 로 서명. workflow 에 `azure/trusted-signing-action` 추가 가능 |
| macOS | Gatekeeper 차단 → 사용자가 `xattr -dr com.apple.quarantine` 또는 우클릭→열기 | Apple Developer ID + notarytool 공증. workflow 에 인증서/시크릿 등록 후 `apple-actions/import-codesign-certs` |

1차 MVP 는 사내 배포 가정이라 미서명으로 두고, 외부 배포 단계에서 추가합니다.

---

## 7. 추가 자료

- API 요청/응답 상세: [`docs/api.md`](docs/api.md)
- 테스트 시나리오: [`docs/test_scenarios.md`](docs/test_scenarios.md)
- 로컬 SQLite 스키마: [`migrations/0001_init.sql`](migrations/0001_init.sql)
- Inno Setup 스크립트: [`installer/pinple.iss`](installer/pinple.iss)
