# 핀플 PC 앱 — 서버 API 명세

PC Agent 가 호출하는 V2 엔드포인트. 모든 본문은 JSON, 모든 시간은 RFC3339 (UTC).

> [!important] 정식 명세는 옵시디언 [[API_명세_핀플_PC_Agent]] 입니다
> 이 문서는 클라(Rust) 개발 편의용 요약입니다. 새 엔드포인트의 상세 스키마·에러 코드는 옵시디언 명세를 우선 참조.

> ## 2026-05-12 일괄 변경 요약 (이 문서 외 정식 옵시디언 명세 동기화 완료)
>
> **신규**
> - **PATCH `/api/pc-agent/policy`** — 회사 관리자 정책 부분 업데이트 (Author≥5)
> - **GET `/api/pc-agent/explanation-types?emp_sid=`** — 회사 커스텀 소명사유 (MVP COMPANY 자동 시드)
> - **CMS 4건**: `/api/cms/pc-agent/explanation-types` POST/PATCH/PATCH-deactivate/usage GET
>
> **변경**
> - GET `/api/pc-agent/policy` — 응답에서 `heartbeat_interval_seconds`, `event_batch_interval_seconds` **제거**. 회사 row 없으면 디폴트 자동 INSERT (멱등).
> - GET `/api/pc-agent/user-info` — 응답에 `explanation_types_version` (epoch) **추가**.
> - POST `/api/pc-agent/worktime-explanations` — `explanation_type` 검증을 DB CHECK → 서비스 룩업으로 (`400 INVALID_EXPLANATION_TYPE`).
> - `event_type` enum — `LOGIN_SUCCESS`/`LOGOUT` **추가**, `HEARTBEAT` **제거** (WITH NOCHECK).
>
> **폐기 단계**
> - POST `/api/pc-agent/heartbeat` — 클라 호출 제거 완료 (Step 3). 서버 라우터/DAO/`PCAGT_HEARTBEAT` 테이블 DROP 은 Step 4 별도.
> - POST `/api/pc-agent/login` / `/refresh` — V1 `check_mbr.jsp` 로 통합 (토큰 미사용). 본 문서 #1/#2 섹션은 history.

| # | Method | Path | 설명 |
|---|--------|------|------|
| 1 | POST | `/api/pc-agent/login` | *(history — 실제로는 V1 `check_mbr.jsp` 사용)* |
| 2 | POST | `/api/pc-agent/refresh` | *(미사용 — 토큰 모델 폐기)* |
| **V1** | **GET** | `/android/u/get_workstatus.jsp?EMPSID=` | **출퇴근 판별 단일 진실 소스** (`result`=Commute SID·0=미출근, `startdt`) — 2026-05-12 |
| 3 | GET  | `/api/pc-agent/policy` | 회사/팀/근로자 정책 조회 (회사 row 없으면 자동 INSERT) |
| **3-1** | **PATCH** | `/api/pc-agent/policy` | **회사 관리자 정책 부분 업데이트** |
| 4 | GET  | `/api/pc-agent/update-check` | 앱 업데이트 확인 |
| 5 | POST | `/api/pc-agent/heartbeat` | *(클라 폐기 — Step 4 라우터 제거 예정)* |
| 6 | POST | `/api/pc-agent/events` | 의미 이벤트 배치 전송 |
| 7 | GET  | `/api/pc-agent/worktime-explanations` | 서버 측 자리비움 / 소명 목록 |
| 8 | POST | `/api/pc-agent/worktime-explanations` | 소명 제출 (룩업 검증) |
| 7' | — | (위 #7 의 응답 정책) | **마감 경과 segment 제외** (2026-05-12) — `EXPLANATION_DEADLINE < 현재 UTC` 는 응답에서 빠짐 |
| 9 | GET  | `/api/pc-agent/attendance-status` | *(deprecated — 10번으로 통합)* |
| 10 | GET  | `/api/pc-agent/user-info` | 유저/요금제/출근 통합 + `explanation_types_version` |
| **11** | **GET** | `/api/pc-agent/explanation-types` | **회사 커스텀 소명사유 목록 (자동 시드)** |
| **12** | **POST** | `/api/cms/pc-agent/explanation-types` | **(CMS) 사유 추가** |
| **13** | **PATCH** | `/api/cms/pc-agent/explanation-types/:sid` | **(CMS) 사유 수정** |
| **14** | **PATCH** | `/api/cms/pc-agent/explanation-types/:sid/deactivate` | **(CMS) 사유 비활성화** |
| **15** | **GET** | `/api/cms/pc-agent/explanation-types/usage` | **(CMS) 사용 통계** |

인증은 매 호출마다 식별자(`emp_sid` / `company_id`) 를 쿼리/본문으로 전달. 토큰 발급/갱신은 사용하지 않으며, 로그인은 V1 `/android/check_mbr.jsp` 의 이메일+비밀번호 검증을 그대로 사용.

CMS 엔드포인트는 `Emply.Author >= 5` (회사 관리자) 권한 검증.

---

## 1. POST `/api/pc-agent/login` *(deprecated — history)*

> [!warning] 2026-05-12 — 이 엔드포인트는 사용되지 않습니다
> 토큰 모델 폐기로 V2 로그인 라우터는 구현되지 않았으며, 실제 로그인은 V1 `GET /android/check_mbr.jsp` (이메일 BASE64 + 비밀번호 SHA-1) 를 사용합니다. 본 섹션은 1차 MVP 설계 시점의 잠정 스펙으로 history 보존.
> 정식 명세: [[API_명세_핀플_PC_Agent]] §1 (`check_mbr.jsp`)

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

## 2. POST `/api/pc-agent/refresh` *(미사용 — 토큰 모델 폐기)*

> [!warning] 2026-05-12 — 사용 안 함
> 토큰 모델 폐기로 refresh 흐름 자체가 없음. 매 호출마다 식별자(`emp_sid` / `company_id`) 동봉. 본 섹션은 history.

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
  "can_track_time": true
}
```

> 2026-05-12 — `heartbeat_interval_seconds` / `event_batch_interval_seconds` 두 필드는 **응답에서 제거**됨 (0005 마이그레이션 DB 컬럼 DROP). 회사 row 가 없을 경우 디폴트 자동 INSERT 후 응답 (멱등).

`policy_scope` 가능값: `COMPANY` / `TEAM` / `EMPLOYEE` / `DEFAULT`.
PC 앱은 `effective_idle_threshold_seconds` 만 idle 판정에 사용하며, 자리비움
구간 저장 시 `applied_idle_threshold_seconds` + `policy_scope` 를 기록한다.

### PATCH `/api/pc-agent/policy` (2026-05-12 신규)

회사 관리자(`Emply.Author≥5`) 가 정책 부분 업데이트. 변경 필드마다 `PCAGT_POLICY_AUDIT` row INSERT. 상세 스키마/입력 범위: [[API_명세_핀플_PC_Agent]] §3-1.

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

## 5. POST `/api/pc-agent/heartbeat` *(deprecated — Step 4 라우터 제거 예정)*

> [!warning] 2026-05-12 — 클라 호출 폐기됨
> heartbeat 책임은 PRESENCE 이벤트(`/events` 로 `LOGIN_SUCCESS`/`LOGOUT`/`PC_SHUTDOWN_DETECTED`) + `user-info` 응답의 `force_logout` 으로 이관됨. 클라이언트(Rust)는 이미 모든 heartbeat 호출 코드를 제거(§12 [[변경_heartbeat_제거_및_PRESENCE_DB]]). 서버 라우터/DAO/`PCAGT_HEARTBEAT` 테이블 DROP 은 Step 4 시점에 별도 진행.
> 본 섹션은 history.

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

> ⚠️ **`can_track_time` 필드는 deprecated** — §부록 B 참조.
> 클라이언트는 이 값을 무시하고 `check_pay_use.jsp` 의 `pinpluse` 만 따른다.

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
`LOGIN_SUCCESS`, `LOGOUT`,
`USER_ACTIVE`, `IDLE_STARTED`, `IDLE_ENDED`, `PC_LOCKED`, `PC_UNLOCKED`,
`PC_SHUTDOWN_DETECTED`, `NO_PC_RECORD`, `EXPLANATION_SUBMITTED`,
`SYNC_SUCCESS`, `SYNC_FAILED`

> 2026-05-12 — `LOGIN_SUCCESS`/`LOGOUT` 추가 (PRESENCE 매핑), `HEARTBEAT` 제거 (heartbeat 엔드포인트 폐기, `WITH NOCHECK` 로 과거 row 보존).

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

> [!warning] 2026-05-12 — 마감 경과 segment 제외
> 응답에서 `EXPLANATION_DEADLINE < 현재 UTC` 인 segment 는 제외됨. 마감이 NULL 인 segment 는 미경과로 간주하여 포함.

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

`explanation_type` 가능값: **회사별로 다름**. 시스템 시드 12개는 `MEETING`, `PHONE_CALL`, `CUSTOMER_RESPONSE`, `BUSINESS_TRIP`, `OUTSIDE_WORK`, `EDUCATION`, `WORK_WAITING`, `PC_ERROR`, `APP_ERROR`, `OTHER_WORK`, `LUNCH_BREAK`, `PERSONAL`. 회사가 자체 코드 추가 가능 — 실제 사용 가능 목록은 [#11 `/explanation-types`](#11-get-apipc-agentexplanation-types) 응답을 참조.

### Response (204)

본문 없음.

### 에러

| Status | 코드/사유 |
|--------|----------|
| 400 | `INVALID_EXPLANATION_TYPE` — 회사 활성 룩업에 없는 코드 (2026-05-12 신규) |
| 400 | segment 없는데 메타도 부족 |
| 403 | 다른 EMPSID 의 segment 에 소명 시도 |
| 404 | segment 없고 메타도 없음 (구 클라이언트) |

---

## 9. GET `/api/pc-agent/attendance-status`

> ⚠️ **Deprecated** — 10번 `/user-info` 응답에 `attendance` 필드로 통합됨.
> 기존 클라이언트 호환을 위해 유지하되, 신규 구현은 `/user-info` 만 사용.

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

## 10. GET `/api/pc-agent/user-info?emp_sid=`

로그인 직후 1회 + 그 이후 **적응형 주기** 로 호출. 유저 / 요금제 / 출근 정보를
한 번에 갱신한다. 기존 V1 `/android/check_pay_use.jsp` (요금제 확인) +
`/android/u/get_main2.jsp` (메인 정보) 의 PC Agent V2 통합본.

### Request (Query)

| 파라미터 | 필수 | 설명 |
|---------|------|------|
| `emp_sid` | ✅ | 근로자 EMPSID (`Emply.Sid`) |

### Response (200)

```json
{
  "user": {
    "employee_id": "48660",
    "employee_name": "홍길동",
    "english_name": "",
    "company_id": "11402",
    "company_name": "성민",
    "team_id": "9869",
    "team_name": "개발",
    "team_template_id": "3221",
    "team_template_name": "성민",
    "position": "",
    "employee_number": "",
    "phone": "01011112222",
    "email": "user@example.com",
    "authority": 5,
    "join_date": "2020-02-02",
    "leave_date": null
  },
  "subscription": {
    "plan_code": "PRO",
    "payment_status": "ACTIVE",
    "pc_tracking_enabled": true,
    "can_track_time": true,
    "valid_until": "2026-12-31"
  },
  "attendance": {
    "attendance_status": "WORKING",
    "work_start_at": "2026-05-07T00:01:23Z",
    "work_end_at": null
  },
  "polled_at": "2026-05-11T06:30:00Z",
  "next_poll_seconds": 3600,
  "force_logout": false,
  "explanation_types_version": 1736654321
}
```

### 응답 필드 설명

- `user.*` : V1 `Emply` / `Mbr` / `Team` / `TopTeam` / `Cmpny` 조인 결과.
  `leave_date` 가 채워져 있으면 클라이언트는 즉시 로그아웃 + 안내.
- `subscription.*` : 회사 단위 PC Agent 사용 권한 **(참고/UI 표시용)**.
  - `plan_code`: `FREE` / `BASIC` / `PRO` / `ENTERPRISE` 등 — UI 결제 정보 표시
  - `payment_status`: `ACTIVE` / `EXPIRED` / `PENDING` — UI 결제 정보 표시
  - `pc_tracking_enabled`: 결제 + 회사 정책 종합 (참고)
  - `can_track_time`: **deprecated** — 추적 ON/OFF 는 `check_pay_use.jsp` 의 `pinpluse` 만 따름 (§부록 B)
- `attendance.*` : 9번과 동일 스키마. 출근 상태 + 시각.
- `polled_at` : 서버 응답 생성 시각 (UTC).
- `next_poll_seconds` : 클라이언트가 다음 호출까지 기다릴 초.
  - `attendance.attendance_status == "WORKING"` → **3600** (1시간)
  - 그 외 (`BEFORE_WORK`/`AFTER_WORK`/`OUTING`/`LEAVE`/`BUSINESS_TRIP`/`UNKNOWN`) → **300** (5분)
- `force_logout` : 서버가 강제 로그아웃을 요청하는 신호 (퇴사자 등).
- `explanation_types_version` *(2026-05-12 신규)* : 회사 커스텀 소명사유 룩업 `MAX(UPD_DT)` UNIX epoch 초. 클라가 캐시한 값과 다르면 [#11 `/explanation-types`](#11-get-apipc-agentexplanation-types) 재호출. 회사 row 없으면 0.

### 동작 정책

1. **호출 시점**
   - 로그인 직후 1회 (블로킹 — UI 가 응답 전까지 "정보 동기화 중" 표시)
   - 이후 `next_poll_seconds` 간격으로 반복
   - **`check_pay_use.jsp` 도 동일 시점에 함께 호출** (요금제 만료/시작 빠른 감지)
2. **응답 변화에 따른 동작**
   - `user.leave_date != null` → 즉시 로그아웃 + 안내
   - `check_pay_use.pinpluse` 토글 → §부록 B 처리 (idle/heartbeat/event 스킵 또는 재개)
   - `attendance` 변경 → idle 감지 ON/OFF 갱신
   - `force_logout = true` → 즉시 로컬 세션 정리 + 로그인 화면 전환

### 404 Not Found

`emp_sid` 가 `Emply` 에 없거나 이미 `LeaveDt` 가 채워진 퇴사자.

---

## 11. GET `/api/pc-agent/explanation-types?emp_sid=` *(2026-05-12 신규)*

회사 커스텀 소명사유 목록. 회사 row 없으면 시스템 시드 12개 자동 INSERT 후 응답. 정식 명세: [[API_명세_핀플_PC_Agent]] §11.

**Response (200)**
```json
{
  "scope": "COMPANY",
  "scope_keys": { "cmpsid": 11402, "ttmsid": 3221, "temsid": 9869 },
  "types": [
    { "exptype_sid": 5, "code": "MEETING",      "label": "회의", "sort_order": 10, "icon": null, "requires_text": false },
    { "exptype_sid": 9, "code": "OUTSIDE_WORK", "label": "외근", "sort_order": 50, "icon": null, "requires_text": true  }
  ],
  "version": 1736654321,
  "seeded": false
}
```

- `exptype_sid` *(2026-05-12 추가)*: CMS PATCH `:exptype_sid/deactivate` / `PATCH :exptype_sid` 호출 시 path 식별자로 사용.
- `version` = `MAX(UPD_DT)` epoch. `user-info.explanation_types_version` 과 비교해서 변경 감지 시 재호출.
- 클라 캐시: `settings` KV (또는 동등). 오프라인 fallback: 시스템 시드 12개 하드코딩 (`OUTSIDE_WORK`/`OTHER_WORK`/`PERSONAL` 만 `requires_text=true`).

---

## 12. POST `/api/cms/pc-agent/explanation-types` *(2026-05-12 신규, 회사 관리자)*

회사 새 사유 추가. 권한: `Emply.Author >= 5`. 상세: [[API_명세_핀플_PC_Agent]] §12.

**Request**: `{ requester_emp_sid, code, label, sort_order?, icon?, requires_text? }`
- `code` 정규식: `^[A-Z][A-Z0-9_]{0,39}$`, 같은 회사 활성 코드 중복 시 `409 DUPLICATE_CODE`
**Response 201**: 생성된 row

---

## 13. PATCH `/api/cms/pc-agent/explanation-types/:exptype_sid` *(2026-05-12 신규, 회사 관리자)*

사유 수정. 권한: `Emply.Author >= 5`. 상세: [[API_명세_핀플_PC_Agent]] §13.

**Request**: `{ requester_emp_sid, patch: { label?, sort_order?, icon?, requires_text? } }`
- `IS_SYSTEM=1` row 는 `label`/`sort_order`/`icon` 만 수정 가능 (`code`/`requires_text` 잠금)
**Response 200**: 변경 후 row

---

## 14. PATCH `/api/cms/pc-agent/explanation-types/:exptype_sid/deactivate` *(2026-05-12 신규, 회사 관리자)*

soft delete. 권한: `Emply.Author >= 5`. 상세: [[API_명세_핀플_PC_Agent]] §14.

**Request**: `{ requester_emp_sid }`
**Response 200**: `is_active:false` 가 된 row
- 활성 셋 ≥ 1 가드: 마지막 1건 시도 시 `409 AT_LEAST_ONE_REQUIRED`

---

## 15. GET `/api/cms/pc-agent/explanation-types/usage?requester_emp_sid=&days=30` *(2026-05-12 신규, 회사 관리자)*

회사 사유 사용 통계. 권한: `Emply.Author >= 5`. 상세: [[API_명세_핀플_PC_Agent]] §15.

**Response 200**
```json
{
  "days": 30,
  "usage": [
    { "code": "MEETING",    "count": 45, "distinct_users": 12 },
    { "code": "PHONE_CALL", "count": 23, "distinct_users": 8 }
  ]
}
```
- `count` desc. 미사용 사유는 응답에 포함되지 않음.
- `days` 범위: `1~365` (기본 30).

---

## 부록 A — `policy_scope` 우선순위

```
employee → team → company → DEFAULT
```

서버는 보통 `effective_idle_threshold_seconds` + `policy_scope` 두 값을 직접
계산해서 내려주지만, 클라이언트는 `monitor::policy::resolve` 로 동일한 계산을
재현할 수 있다.

## 부록 B — 요금제 권한 판별 (단일 결정자: `check_pay_use.jsp`)

PC Agent 의 추적 기능 사용 여부는 **V1 `GET /android/check_pay_use.jsp?CMPSID=&MBRSID=`
응답의 `pinpluse` 필드** 가 단일 결정자다.

| `pinpluse` | 결과 |
|------------|------|
| `true`     | 추적 가능 — 정상 동작 |
| `false`    | 추적 불가 — Disabled 화면 |

**호출 시점**
- 로그인 직후 1회
- `GET /user-info` 폴링 (1h / 5min) 과 같은 주기로 함께 호출 (요금제 만료/시작 빠른 감지)

**`pinpluse = false` 일 때 PC 앱 동작**
- 로그인은 유지 (Disabled 화면 표시 + "요금제 만료" 안내)
- idle 감지 루프 skip
- heartbeat 전송 skip
- 이벤트 enqueue 자체는 허용하되 배치 전송 skip

서버 측에서도 이 상태에서 들어오는 이벤트는 저장하지 않아야 한다.

### V2 응답의 `can_track_time` 필드 의미 변경

다음 필드들은 **요금제 결정에 더 이상 사용되지 않으며, 참고/디버깅용 정보**다.
클라이언트는 무시한다.

| 위치 | 용도 변화 |
|------|-----------|
| `heartbeat` 응답 `can_track_time` | **deprecated** — 정책 측 토글 정보 표시용. 추적 ON/OFF 는 `check_pay_use` 만 따른다. |
| `user-info` 응답 `subscription.can_track_time` | **참고용** — UI 에 "결제 상태" 표시에만 사용. 실제 결정은 `check_pay_use`. |
| `PCAGT_POLICY.CAN_TRACK_TIME` (DB) | **운영자 수동 차단 오버라이드** — true 라도 `check_pay_use=false` 면 차단. |

이전 (v1.0) 설계에서는 위 세 가지가 모두 결정에 참여했으나, 결정 로직 불일치
위험을 없애기 위해 단일 결정자로 통합했다.

## 부록 C — `user-info` 적응형 폴링 주기

```
attendance_status        next_poll_seconds
─────────────────────    ──────────────────
WORKING                  3600  (1시간)
BEFORE_WORK              300   (5분)
AFTER_WORK               300   (5분)
OUTING                   300   (5분)
LEAVE                    300   (5분)
BUSINESS_TRIP            300   (5분)
UNKNOWN                  300   (5분)
```

설계 의도:
- **근무 중**: 1시간이면 충분 — 출근 상태가 자주 바뀔 일이 적음.
- **근무 외**: 5분 — 출근 누르면 5분 안에 PC Agent 가 감지해서 idle 추적 시작.

클라이언트는 응답의 `next_poll_seconds` 값을 그대로 사용하고, 서버는 운영
필요에 따라 회사별 / 시간대별로 값을 조정할 수 있다 (응답 즉시 반영).
