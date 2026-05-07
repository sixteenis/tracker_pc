# 핀플 PC 앱 — 서버 API 명세 (1차 MVP)

PC Agent 가 호출하는 9 개 엔드포인트. 모든 본문은 JSON, 모든 시간은 RFC3339 (UTC).

| # | Method | Path | 설명 |
|---|--------|------|------|
| 1 | POST | `/api/pc-agent/login` | 근로자 로그인 |
| 2 | POST | `/api/pc-agent/refresh` | refresh_token 으로 access_token 재발급 |
| 3 | GET  | `/api/pc-agent/policy` | 회사/팀/근로자 정책 조회 |
| 4 | GET  | `/api/pc-agent/update-check` | 앱 업데이트 확인 |
| 5 | POST | `/api/pc-agent/heartbeat` | 3분 주기 상태 보고 |
| 6 | POST | `/api/pc-agent/events` | 의미 이벤트 배치 전송 |
| 7 | GET  | `/api/pc-agent/worktime-explanations` | 서버 측 자리비움 / 소명 목록 |
| 8 | POST | `/api/pc-agent/worktime-explanations` | 소명 제출 |
| 9 | GET  | `/api/pc-agent/attendance-status` | 오늘 출근 상태 조회 |

인증은 5/6/7/8/9 모두 `Authorization: Bearer <access_token>` 헤더 사용.

---

## 1. POST `/api/pc-agent/login`

### Request

```json
{
  "login_id": "user@example.com",
  "password": "...",
  "device_id": "f6c8b1c3-4a5d-4d2f-9d3a-...",
  "device_name": "DESKTOP-ABC (Windows)",
  "app_version": "0.1.0"
}
```

### Response (200)

```json
{
  "access_token": "eyJhbGciOi...",
  "refresh_token": "rt_2026_05_07_...",
  "access_token_expires_in": 3600,

  "company_id": "CO_001",
  "employee_id": "EMP_1234",
  "employee_name": "홍길동",
  "team_id": "TEAM_DEV",
  "team_name": "개발팀",

  "subscription": {
    "plan_code": "PRO",
    "payment_status": "ACTIVE",
    "pc_tracking_enabled": true,
    "can_track_time": true
  },

  "policy": {
    "policy_version": 7,
    "company_idle_threshold_seconds": 600,
    "team_idle_threshold_seconds": 900,
    "employee_idle_threshold_seconds": null,
    "effective_idle_threshold_seconds": 900,
    "policy_scope": "TEAM",
    "lunch_start_time": "11:30",
    "lunch_end_time": "14:00",
    "lunch_allowed_minutes": 60,
    "explanation_deadline_hours": 48,
    "heartbeat_interval_seconds": 180,
    "event_batch_interval_seconds": 60,
    "can_track_time": true
  },

  "displaced_device": {
    "device_id": "old-device-uuid",
    "device_name": "DESKTOP-OLD (Windows)",
    "displaced_at": "2026-05-06T22:14:01Z"
  }
}
```

`displaced_device` 는 다른 PC 에서 활성 로그인이었을 때만 채워진다. 서버는 본
요청 처리 시 이미 기존 device 를 비활성화한다.

---

## 2. POST `/api/pc-agent/refresh`

### Request

```json
{
  "refresh_token": "rt_2026_05_07_...",
  "device_id": "f6c8b1c3-4a5d-4d2f-9d3a-...",
  "device_name": "DESKTOP-ABC (Windows)",
  "app_version": "0.1.0"
}
```

### Response (200)

위 1번 Login 응답과 동일한 스키마.

### 401 Unauthorized

refresh_token 만료 — 클라이언트는 로그인 화면으로 전환.

---

## 3. GET `/api/pc-agent/policy`

### Response (200)

```json
{
  "policy_version": 7,
  "company_idle_threshold_seconds": 600,
  "team_idle_threshold_seconds": 900,
  "employee_idle_threshold_seconds": null,
  "effective_idle_threshold_seconds": 900,
  "policy_scope": "TEAM",
  "lunch_start_time": "11:30",
  "lunch_end_time": "14:00",
  "lunch_allowed_minutes": 60,
  "explanation_deadline_hours": 48,
  "heartbeat_interval_seconds": 180,
  "event_batch_interval_seconds": 60,
  "can_track_time": true
}
```

`policy_scope` 가능값: `COMPANY` / `TEAM` / `EMPLOYEE` / `DEFAULT`.
PC 앱은 `effective_idle_threshold_seconds` 만 idle 판정에 사용하며, 자리비움
구간 저장 시 `applied_idle_threshold_seconds` + `policy_scope` 를 기록한다.

---

## 4. GET `/api/pc-agent/update-check?current_version=0.1.0&os=windows`

### Response (200)

```json
{
  "current_version": "0.1.0",
  "latest_version": "0.2.1",
  "minimum_required_version": "0.2.0",
  "update_required": true,
  "force_update": true,
  "download_url": "https://cdn.pinple.io/pcagent/PinplePCAgent_Setup_0_2_1.exe",
  "release_note": "보안 패치 및 정책 동기화 개선"
}
```

`force_update = true` 이고 `current_version < minimum_required_version` 이면
PC 기록 기능을 즉시 정지하고 업데이트를 유도한다.

---

## 5. POST `/api/pc-agent/heartbeat`

### Request

```json
{
  "company_id": "CO_001",
  "employee_id": "EMP_1234",
  "device_id": "f6c8b1c3-...",
  "device_name": "DESKTOP-ABC (Windows)",
  "app_version": "0.1.0",

  "pc_status": "ACTIVE",
  "last_activity_at": "2026-05-07T08:23:11Z",
  "idle_seconds": 0,
  "is_locked": false,

  "attendance_status": "WORKING",
  "can_track_time": true,
  "effective_idle_threshold_seconds": 900
}
```

`pc_status` 가능값: `ACTIVE` / `IDLE` / `LOCKED` / `APP_CLOSING` / `OFFLINE`.

### Response (200)

```json
{
  "next_heartbeat_seconds": 180,
  "policy_version": 7,
  "can_track_time": true,
  "force_logout": false
}
```

`force_logout = true` 면 클라이언트는 즉시 로컬 세션을 비우고 로그인 화면으로 전환.

---

## 6. POST `/api/pc-agent/events`

### Request

```json
{
  "company_id": "CO_001",
  "employee_id": "EMP_1234",
  "device_id": "f6c8b1c3-...",
  "events": [
    {
      "event_id": "9c2c2bf0-3f1e-4d9a-...",
      "event_type": "IDLE_STARTED",
      "event_time": "2026-05-07T03:50:00Z",
      "payload": {
        "segment_id": "seg-uuid",
        "started_at": "2026-05-07T03:35:00Z",
        "applied_idle_threshold_seconds": 900,
        "policy_scope": "TEAM"
      }
    },
    {
      "event_id": "ad1c2b...",
      "event_type": "IDLE_ENDED",
      "event_time": "2026-05-07T04:14:23Z",
      "payload": { "segment_id": "seg-uuid", "ended_at": "2026-05-07T04:14:23Z" }
    }
  ]
}
```

### 지원하는 `event_type`

`APP_STARTED`, `APP_STOPPED`, `AUTO_LOGIN_SUCCESS`, `AUTO_LOGIN_FAILED`,
`USER_ACTIVE`, `IDLE_STARTED`, `IDLE_ENDED`, `PC_LOCKED`, `PC_UNLOCKED`,
`PC_SHUTDOWN_DETECTED`, `NO_PC_RECORD`, `HEARTBEAT`, `EXPLANATION_SUBMITTED`,
`SYNC_SUCCESS`, `SYNC_FAILED`.

### Response (200)

```json
{
  "accepted_event_ids": ["9c2c2bf0-3f1e-4d9a-...", "ad1c2b..."]
}
```

서버는 `event_id` 기준으로 멱등 처리한다 — 중복 전송된 이벤트는 무시되며
`accepted_event_ids` 에는 그대로 포함되어 반환된다.

---

## 7. GET `/api/pc-agent/worktime-explanations`

### Response (200)

```json
[
  {
    "segment_id": "seg-uuid",
    "work_date": "2026-05-07",
    "start_time": "2026-05-07T03:50:00Z",
    "end_time": "2026-05-07T04:46:00Z",
    "duration_seconds": 3360,
    "segment_type": "PC_IDLE",
    "applied_idle_threshold_seconds": 900,
    "explanation_deadline": "2026-05-09T03:50:00Z",
    "explanation_status": "PENDING"
  }
]
```

PC 앱은 이 응답을 로컬 `idle_segments` 와 병합해서 표시한다 (1차 MVP 에서는
주로 로컬 segment 만 표시).

---

## 8. POST `/api/pc-agent/worktime-explanations`

### Request

```json
{
  "segment_id": "seg-uuid",
  "explanation_type": "MEETING",
  "explanation_text": "임원 미팅으로 자리 비움",
  "submitted_from": "PC_APP"
}
```

`explanation_type` 가능값: `MEETING`, `PHONE_CALL`, `CUSTOMER_RESPONSE`,
`BUSINESS_TRIP`, `OUTSIDE_WORK`, `EDUCATION`, `WORK_WAITING`, `PC_ERROR`,
`APP_ERROR`, `OTHER_WORK`, `LUNCH_BREAK`, `PERSONAL`.

### Response (204)

본문 없음.

---

## 9. GET `/api/pc-agent/attendance-status`

### Response (200)

```json
{
  "attendance_status": "WORKING",
  "work_start_at": "2026-05-07T00:01:23Z",
  "work_end_at": null
}
```

`attendance_status` 가능값: `WORKING`, `BEFORE_WORK`, `AFTER_WORK`, `OUTING`,
`LEAVE`, `BUSINESS_TRIP`, `UNKNOWN`.

---

## 부록 A — `policy_scope` 우선순위

```
employee → team → company → DEFAULT
```

서버는 보통 `effective_idle_threshold_seconds` + `policy_scope` 두 값을 직접
계산해서 내려주지만, 클라이언트는 `monitor::policy::resolve` 로 동일한 계산을
재현할 수 있다.

## 부록 B — 요금제 미포함 처리

`subscription.can_track_time = false` 또는 `policy.can_track_time = false` 인
경우 PC 앱은:

- 로그인은 유지 (Disabled 화면 표시)
- idle 감지 루프 skip
- heartbeat 전송 skip
- 이벤트 enqueue 자체는 허용하되 배치 전송 skip

서버 측에서도 이 상태에서 들어오는 이벤트는 저장하지 않아야 한다.
