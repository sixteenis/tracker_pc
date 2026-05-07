# 핀플 PC 앱 1차 MVP — 테스트 시나리오

기획서 §24 의 "테스트 시나리오" 출력물. Mock 모드 (`api.mock_mode = true`) 와
실서버 연결 두 환경 모두에서 동일하게 수행한다.

> **사전 준비**: `cargo run` 으로 앱 실행. 로컬 SQLite 는 OS 사용자 데이터
> 디렉토리(예: `~/Library/Application Support/Pinple/PCAgent/pinple.db`)에 생성된다.

---

## TS-01. 로그인 / 로그아웃

| 단계 | 입력 | 기대 결과 |
|------|------|-----------|
| 1 | 앱 실행 | 로그인 화면 노출, 한글 폰트 적용 |
| 2 | 아이디/비번 입력 후 "로그인" | 상태 화면으로 전환, 출근 상태 / 정책 / can_track_time 표시 |
| 3 | "자동로그인" 체크 후 로그인 | OS Credential Store 에 refresh_token 저장 (macOS Keychain Access 에서 `io.pinple.pcagent` 항목 확인 가능) |
| 4 | 앱 재시작 | 로그인 화면을 거치지 않고 자동로그인 → 상태 화면 |
| 5 | "로그아웃" 버튼 | refresh_token 제거, `auth` 테이블 비워짐, 로그인 화면으로 |

**검증 SQL** — 비밀번호가 절대 저장되지 않는지 확인:

```bash
sqlite3 "$(rust 로그에서 보이는 DB 경로)" "SELECT * FROM auth;"
sqlite3 "$(...)" ".schema"  # auth 테이블에 password 컬럼 없는지
```

---

## TS-02. 요금제 미포함 (`can_track_time = false`)

| 단계 | 조건 | 기대 결과 |
|------|------|-----------|
| 1 | Mock 응답을 `can_track_time = false` 로 임시 수정 (`src/api/mock.rs::fake_policy`) | 로그인은 성공하지만 즉시 `Disabled` 화면 노출 |
| 2 | idle 감지 / heartbeat 전송 | 백그라운드 로그에 `heartbeat skip`, idle segment 미생성 |
| 3 | 화면 문구 | "PC 근무활동 기록 기능이 비활성화되어 있습니다 …" |

---

## TS-03. 자리비움 감지 (idle)

| 단계 | 조작 | 기대 결과 |
|------|------|-----------|
| 1 | 환경변수 `PINPLE_FAKE_IDLE=700 cargo run` (macOS/Linux 개발용) | 700초 idle 인 것처럼 동작 |
| 2 | 5초 후 | `idle_segments` 에 `segment_type=PC_IDLE` 한 건, `applied_idle_threshold_seconds=600`, `policy_scope=COMPANY` 저장 |
| 3 | `local_events` 에 `IDLE_STARTED` 한 건 enqueue | 1분 내 Mock API 로 SUCCESS 처리 (`sync_status='SUCCESS'`) |
| 4 | `PINPLE_FAKE_IDLE=0` 으로 재실행 | `IDLE_ENDED` 이벤트 enqueue, segment 의 `end_time / duration_seconds` 채워짐 |

Windows 실 환경에서는 5분 이상 키보드/마우스 입력을 하지 않고 대기.

---

## TS-04. 정책 우선순위 (`policy_scope`)

`cargo test monitor::policy::tests` 로 4 개 케이스 자동 검증:

```
employee_wins                — employee 600/team 900/company 600 → 600 (EMPLOYEE)
team_when_no_employee         — team 900/company 600 → 900 (TEAM)
company_when_no_team          — company 600 → 600 (COMPANY)
default_when_nothing          — 모두 None → fallback (DEFAULT)
```

또한 Mock 응답을 다음과 같이 수정해 UI 에서 적용 결과를 육안 확인:

```jsonc
{
  "company_idle_threshold_seconds": 600,
  "team_idle_threshold_seconds": 900,
  "employee_idle_threshold_seconds": null,
  "effective_idle_threshold_seconds": 900,
  "policy_scope": "TEAM"
}
```

→ 상태 화면 "자리비움 기준" 행에 `15분 0초 (TEAM)` 표시.

---

## TS-05. 점심 윈도우

`cargo test lunch::tests` 로 3 케이스 자동 검증:

| 케이스 | 입력 (로컬) | 분류 |
|--------|-------------|------|
| 점심 안 + 인정시간 이하 | 12:00–12:45 | `LunchCandidate` |
| 점심 안 + 인정시간 초과 | 12:00–13:30 | `LunchExceeded { exceeded = 30분 }` |
| 점심 밖 | 15:00–15:30 | `Outside` |

---

## TS-06. 이벤트 배치 / 재전송

| 단계 | 조작 | 기대 결과 |
|------|------|-----------|
| 1 | 강제로 idle segment 생성 (TS-03) | `local_events` 에 `IDLE_STARTED` row 1개, `sync_status='PENDING'` |
| 2 | 1분 대기 | Mock API 정상 응답 → row `SUCCESS` 로 갱신, `synced_at` 채워짐 |
| 3 | 실서버 모드 + 네트워크 단절 (`PINPLE_API_BASE_URL=http://127.0.0.1:1` 등) | row `FAILED`, `retry_count++`, `last_error` 기록 |
| 4 | 네트워크 복구 후 1분 대기 | 같은 row 가 `SUCCESS` 로 전환 (멱등 — `event_id` 중복 거부) |

**SQL 점검**:

```sql
SELECT event_type, sync_status, retry_count, last_error
FROM local_events
ORDER BY id DESC LIMIT 20;
```

---

## TS-07. 소명 입력 / 제출

| 단계 | 조작 | 기대 결과 |
|------|------|-----------|
| 1 | TS-03 로 idle segment 생성 | "근무시간 소명" 화면에서 1건 노출, OS 토스트 1회 표시 |
| 2 | 소명 입력 (사유: MEETING, 내용: "임원 미팅") → "제출" | `explanations` row 1개, `idle_segments.explanation_status = SUBMITTED` |
| 3 | Mock 응답 정상 | 비동기로 서버 전송 → `explanations.sync_status='SUCCESS'` |
| 4 | 동일 segment 재진입 | 목록에서 사라짐 |

---

## TS-08. NO_PC_RECORD (앱 비정상 종료 후 재기동)

| 단계 | 조작 | 기대 결과 |
|------|------|-----------|
| 1 | 앱 정상 실행 후 heartbeat 1회 성공 | `settings.last_heartbeat_at` 갱신 |
| 2 | 앱 강제 종료 후 30분 이상 대기 (또는 `settings` 의 시각을 SQL 로 과거로 수정) | — |
| 3 | 앱 재기동 | `idle_segments` 에 `segment_type=NO_PC_RECORD` 1건, `local_events` 에 동명 이벤트 |
| 4 | "근무시간 소명" 화면 | NO_PC_RECORD 항목 노출, 소명 가능 |

---

## TS-09. 업데이트 강제 (force_update)

`src/api/mock.rs::update_check` 를 임시로 다음과 같이 수정:

```rust
Ok(UpdateInfo {
    current_version: req.current_version.clone(),
    latest_version: "9.9.9".into(),
    minimum_required_version: "9.9.9".into(),
    update_required: true,
    force_update: true,
    download_url: "https://example.invalid/setup.exe".into(),
    release_note: "테스트 강제 업데이트".into(),
})
```

| 단계 | 기대 결과 |
|------|-----------|
| 1 | 앱 실행 후 15초 대기 | "업데이트" 메뉴 등장, `can_track_time = false` 강제 전환 |
| 2 | "업데이트" 메뉴 진입 | 다운로드 링크 표시 |

---

## TS-10. 출근 상태 변화 (스마트폰 앱 시뮬)

Mock 모드에서 `attendance_sync` 가 항상 `WORKING` 을 반환하므로,
실서버 모드에서 출근 상태를 `BEFORE_WORK` 등으로 바꾸어 5분 대기.

| `attendance_status` | 기대 동작 |
|---------------------|-----------|
| `WORKING` | idle 감지 ON |
| `BEFORE_WORK` / `AFTER_WORK` / `OUTING` / `LEAVE` / `BUSINESS_TRIP` | idle 감지 OFF (segment 생성 안함) |
| `UNKNOWN` | idle 감지 ON (안전 기본값) |

---

## TS-11. 1근로자 1PC 정책

| 단계 | 조작 | 기대 결과 |
|------|------|-----------|
| 1 | PC-A 에서 로그인 | 정상 |
| 2 | PC-B 에서 같은 계정 로그인 | 서버가 `displaced_device` 를 채워 응답, PC-A 의 다음 heartbeat 응답에 `force_logout = true` |
| 3 | PC-A | 자동 로그아웃 → 로그인 화면 |

(Mock 모드에서는 `force_logout = false` 만 반환하므로 본 시나리오는 실서버에서만 검증)

---

## TS-12. 보안 감사

| 점검 항목 | 방법 |
|----------|------|
| 비밀번호가 DB / 로그 / 파일에 남지 않음 | `grep -ri "password" target/ Cargo.lock | grep -v "set_password"` 후 결과 검토. SQLite 의 `auth` 테이블 컬럼 확인 |
| `access_token` 디스크 저장 없음 | DB / 설정 파일 어디에도 평문 저장 안됨 — 메모리 한정 |
| `refresh_token` OS Credential Store 사용 | macOS: Keychain Access → `io.pinple.pcagent` / Windows: `cmdkey /list` |
| 키/마우스 내용이 서버 페이로드에 포함되지 않음 | `payload_json` 필드를 무작위 추출해서 키 입력 텍스트가 들어있지 않은지 확인 |
| 화면 캡처 / 프로세스 목록 / URL 미수집 | 의존성 트리(`cargo tree`)에 `screenshots`, `sysinfo` 등 관련 크레이트 없음 |

---

## 자동화된 단위 테스트

```bash
cargo test
```

8 개 테스트 통과:

- `monitor::policy::tests::*` (4) — 우선순위 해석
- `lunch::tests::*` (3) — 점심 윈도우 분류
- `sync::update_check::tests::lt_basic` (1) — 버전 비교
