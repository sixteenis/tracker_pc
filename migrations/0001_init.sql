-- 핀플 PC 앱 로컬 SQLite 스키마 (1차 MVP)
-- 모든 시간은 ISO 8601 UTC 문자열 또는 epoch 초로 저장.
-- 비밀번호는 절대 저장하지 않음. 토큰은 OS Credential Store 에 별도 보관.

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

-- 1. auth: 로그인 식별 정보 (토큰 제외)
CREATE TABLE IF NOT EXISTS auth (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id        TEXT    NOT NULL,
    employee_id       TEXT    NOT NULL,
    employee_name     TEXT,
    team_id           TEXT,
    team_name         TEXT,
    device_id         TEXT    NOT NULL,
    device_name       TEXT    NOT NULL,
    auto_login        INTEGER NOT NULL DEFAULT 0,
    last_login_at     TEXT
);

-- 2. local_events: 서버 전송 대상 이벤트 큐 (PENDING/SUCCESS/FAILED)
CREATE TABLE IF NOT EXISTS local_events (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id      TEXT    NOT NULL UNIQUE,
    event_type    TEXT    NOT NULL,
    event_time    TEXT    NOT NULL,
    payload_json  TEXT    NOT NULL,
    sync_status   TEXT    NOT NULL DEFAULT 'PENDING',
    retry_count   INTEGER NOT NULL DEFAULT 0,
    last_error    TEXT,
    created_at    TEXT    NOT NULL,
    synced_at     TEXT
);
CREATE INDEX IF NOT EXISTS idx_local_events_status ON local_events(sync_status, created_at);

-- 3. idle_segments: 자리비움/잠금/앱종료/PC종료/기록없음 구간
CREATE TABLE IF NOT EXISTS idle_segments (
    id                              INTEGER PRIMARY KEY AUTOINCREMENT,
    segment_id                      TEXT    NOT NULL UNIQUE,
    company_id                      TEXT    NOT NULL,
    employee_id                     TEXT    NOT NULL,
    device_id                       TEXT    NOT NULL,
    work_date                       TEXT    NOT NULL,
    segment_type                    TEXT    NOT NULL,  -- PC_IDLE / PC_LOCKED / PC_APP_CLOSED / PC_SHUTDOWN / NO_PC_RECORD
    start_time                      TEXT    NOT NULL,
    end_time                        TEXT,
    duration_seconds                INTEGER,
    applied_idle_threshold_seconds  INTEGER NOT NULL,
    policy_scope                    TEXT    NOT NULL,  -- COMPANY / TEAM / EMPLOYEE / DEFAULT
    explanation_required            INTEGER NOT NULL DEFAULT 1,
    explanation_deadline            TEXT,
    explanation_status              TEXT    NOT NULL DEFAULT 'PENDING', -- PENDING / SUBMITTED / EXPIRED / EXEMPTED
    worktime_reflection_status      TEXT,    -- BREAK_CANDIDATE / WORK_CONFIRMED / etc.
    created_at                      TEXT    NOT NULL,
    updated_at                      TEXT    NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_idle_segments_employee_date ON idle_segments(employee_id, work_date);
CREATE INDEX IF NOT EXISTS idx_idle_segments_status ON idle_segments(explanation_status);

-- 4. explanations: 근로자가 입력한 소명 내용
CREATE TABLE IF NOT EXISTS explanations (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    segment_id          TEXT    NOT NULL,
    work_date           TEXT    NOT NULL,
    start_time          TEXT    NOT NULL,
    end_time            TEXT    NOT NULL,
    duration_seconds    INTEGER NOT NULL,
    explanation_type    TEXT    NOT NULL,  -- MEETING / PHONE_CALL / ... / PERSONAL
    explanation_text    TEXT,
    submitted_from      TEXT    NOT NULL DEFAULT 'PC_APP',
    submitted_at        TEXT    NOT NULL,
    sync_status         TEXT    NOT NULL DEFAULT 'PENDING',
    FOREIGN KEY (segment_id) REFERENCES idle_segments(segment_id)
);
CREATE INDEX IF NOT EXISTS idx_explanations_segment ON explanations(segment_id);

-- 5. settings: 단순 key/value (정책 캐시, 마지막 동기화 시각 등)
CREATE TABLE IF NOT EXISTS settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
