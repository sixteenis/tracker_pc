-- ============================================================================
-- 핀플 PC Agent V2 — 운영자용 KST 뷰 일괄 신설 (2026-05-13)
-- ----------------------------------------------------------------------------
-- 배경:
--   PCAGT_* 테이블의 시간 컬럼은 UTC `DATETIME2(0)` 로 저장된다 (글로벌 표준).
--   사용자/UI 는 클라이언트 측 `format_local_time` 으로 자동 KST 변환되어
--   문제 없지만, 운영자가 SSMS / sqlcmd 로 DB 를 직접 조회할 때 UTC 시각이
--   보여 한국 시간 감각과 9시간 어긋난다는 운영 불편이 있었다.
--
--   기존 V1 테이블(`Mbr.RegDt` 등)은 KST 저장이라 두 정책이 공존 →
--   운영자가 V1+V2 join 분석할 때도 혼란.
--
-- 결정 (2026-05-13, 방안 5 채택):
--   - 저장 정책은 그대로 UTC 유지 (클라 / 서버 / API contract 변경 없음)
--   - 운영자가 보는 KST 시야는 별도 뷰로 제공
--   - 10개 PCAGT_* 테이블 각각에 대응되는 `v_PCAGT_*_KST` 뷰 생성
--
-- 명명 규칙:
--   - 원본 테이블: `dbo.PCAGT_*` (UTC)
--   - KST 뷰:    `dbo.v_PCAGT_*_KST` (시간 컬럼 +9시간 변환된 결과)
--
-- 사용법 (운영자):
--   SELECT * FROM dbo.v_PCAGT_IDLE_SEGMENT_KST WHERE EMPSID = 48660;
--   ← START_TIME / END_TIME 등이 KST 로 표시됨
--
-- 코드 영향:
--   - 클라 (Rust pinple_pc_agent): 영향 없음 — 원본 테이블만 사용
--   - 서버 V2 (Node.js): 영향 없음 — 원본 테이블만 사용
--   - API contract: 영향 없음 — RFC3339 UTC 그대로
--
-- 처리 규칙:
--   - datetime2 컬럼: `DATEADD(hour, 9, x)` 변환
--   - date 컬럼 (WORK_DATE): 그대로 — 날짜만이라 timezone 변환 의미 없음
--   - NULL 값: DATEADD 결과도 NULL (자연 처리)
--
-- 멱등성: `CREATE OR ALTER VIEW` — 재실행 안전.
-- ============================================================================

SET NOCOUNT ON;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 1) v_PCAGT_DEVICE_SESSION_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_DEVICE_SESSION_KST AS
SELECT
    SESSION_SID, CMPSID, MBRSID, EMPSID, TTMSID, TEMSID,
    DEVICE_ID, DEVICE_NAME, OS, APP_VERSION,
    IS_ACTIVE,
    DATEADD(hour, 9, DISPLACED_AT)      AS DISPLACED_AT,
    DISPLACED_REASON,
    DATEADD(hour, 9, LAST_LOGIN_AT)     AS LAST_LOGIN_AT,
    DATEADD(hour, 9, LAST_HEARTBEAT_AT) AS LAST_HEARTBEAT_AT,
    DATEADD(hour, 9, REG_DT)            AS REG_DT,
    DATEADD(hour, 9, UPD_DT)            AS UPD_DT
FROM dbo.PCAGT_DEVICE_SESSION;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) v_PCAGT_POLICY_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_POLICY_KST AS
SELECT
    POLICY_SID, POLICY_SCOPE, CMPSID, TTMSID, TEMSID, EMPSID,
    IDLE_THRESHOLD_SECONDS,
    LUNCH_START_TIME, LUNCH_END_TIME, LUNCH_ALLOWED_MINUTES,
    EXPLANATION_DEADLINE_HOURS,
    CAN_TRACK_TIME, POLICY_VERSION, IS_ACTIVE,
    DATEADD(hour, 9, REG_DT) AS REG_DT,
    DATEADD(hour, 9, UPD_DT) AS UPD_DT
FROM dbo.PCAGT_POLICY;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 3) v_PCAGT_APP_VERSION_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_APP_VERSION_KST AS
SELECT
    APPVER_SID, OS, LATEST_VERSION, MINIMUM_REQUIRED_VERSION,
    FORCE_UPDATE, DOWNLOAD_URL, RELEASE_NOTE, IS_ACTIVE,
    DATEADD(hour, 9, REG_DT) AS REG_DT,
    DATEADD(hour, 9, UPD_DT) AS UPD_DT
FROM dbo.PCAGT_APP_VERSION;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 4) v_PCAGT_HEARTBEAT_KST  (heartbeat 폐기 진행 중이지만 운영 호환 위해 포함)
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_HEARTBEAT_KST AS
SELECT
    HB_SID, CMPSID, EMPSID, DEVICE_ID, DEVICE_NAME, APP_VERSION,
    PC_STATUS,
    DATEADD(hour, 9, LAST_ACTIVITY_AT) AS LAST_ACTIVITY_AT,
    IDLE_SECONDS, IS_LOCKED, ATTENDANCE_STATUS, CAN_TRACK_TIME,
    EFFECTIVE_IDLE_THRESHOLD_SECONDS,
    DATEADD(hour, 9, REG_DT) AS REG_DT
FROM dbo.PCAGT_HEARTBEAT;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 5) v_PCAGT_EVENT_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_EVENT_KST AS
SELECT
    EVENT_SID, EVENT_ID, CMPSID, EMPSID, DEVICE_ID,
    EVENT_TYPE,
    DATEADD(hour, 9, EVENT_TIME)  AS EVENT_TIME,
    PAYLOAD_JSON,
    DATEADD(hour, 9, RECEIVED_AT) AS RECEIVED_AT
FROM dbo.PCAGT_EVENT;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 6) v_PCAGT_IDLE_SEGMENT_KST
--    (운영자가 가장 자주 보는 테이블 — 자리비움 segment)
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_IDLE_SEGMENT_KST AS
SELECT
    SEGMENT_ID, CMPSID, EMPSID, DEVICE_ID,
    WORK_DATE,                                   -- DATE — KST 가정 그대로
    SEGMENT_TYPE,
    DATEADD(hour, 9, START_TIME)           AS START_TIME,
    DATEADD(hour, 9, END_TIME)             AS END_TIME,
    DURATION_SECONDS,
    APPLIED_IDLE_THRESHOLD_SECONDS, POLICY_SCOPE,
    EXPLANATION_REQUIRED,
    DATEADD(hour, 9, EXPLANATION_DEADLINE) AS EXPLANATION_DEADLINE,
    EXPLANATION_STATUS, WORKTIME_REFLECTION_STATUS,
    DATEADD(hour, 9, REG_DT)               AS REG_DT,
    DATEADD(hour, 9, UPD_DT)               AS UPD_DT
FROM dbo.PCAGT_IDLE_SEGMENT;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 7) v_PCAGT_EXPLANATION_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_EXPLANATION_KST AS
SELECT
    EXP_SID, SEGMENT_ID, CMPSID, EMPSID,
    EXPLANATION_TYPE, EXPLANATION_TEXT, OTHER_TYPE_LABEL, SUBMITTED_FROM,
    DATEADD(hour, 9, SUBMITTED_AT) AS SUBMITTED_AT,
    DATEADD(hour, 9, REG_DT)       AS REG_DT
FROM dbo.PCAGT_EXPLANATION;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 8) v_PCAGT_ATTENDANCE_SNAPSHOT_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_ATTENDANCE_SNAPSHOT_KST AS
SELECT
    ATT_SID, CMPSID, EMPSID,
    WORK_DATE,                              -- DATE — KST 가정 그대로
    ATTENDANCE_STATUS,
    DATEADD(hour, 9, WORK_START_AT) AS WORK_START_AT,
    DATEADD(hour, 9, WORK_END_AT)   AS WORK_END_AT,
    SOURCE,
    DATEADD(hour, 9, REG_DT)        AS REG_DT,
    DATEADD(hour, 9, UPD_DT)        AS UPD_DT
FROM dbo.PCAGT_ATTENDANCE_SNAPSHOT;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 9) v_PCAGT_EXPLANATION_TYPE_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_EXPLANATION_TYPE_KST AS
SELECT
    EXPTYPE_SID, SCOPE, CMPSID, TTMSID, TEMSID,
    CODE, LABEL, SORT_ORDER, ICON,
    REQUIRES_TEXT, IS_SYSTEM, IS_PROTECTED, IS_ACTIVE,
    DATEADD(hour, 9, REG_DT) AS REG_DT,
    DATEADD(hour, 9, UPD_DT) AS UPD_DT
FROM dbo.PCAGT_EXPLANATION_TYPE;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 10) v_PCAGT_POLICY_AUDIT_KST
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_POLICY_AUDIT_KST AS
SELECT
    AUDIT_SID, CMPSID, EMPSID,
    DATEADD(hour, 9, CHANGED_AT) AS CHANGED_AT,
    FIELD_NAME, OLD_VALUE, NEW_VALUE, REASON,
    DATEADD(hour, 9, REG_DT)     AS REG_DT
FROM dbo.PCAGT_POLICY_AUDIT;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 검증 쿼리 (적용 후 실행)
-- ────────────────────────────────────────────────────────────────────────────
-- 1) 10개 뷰가 모두 생성됐는지
-- SELECT name FROM sys.views WHERE name LIKE 'v_PCAGT[_]%[_]KST' ORDER BY name;
--
-- 2) 원본 vs KST 뷰 — START_TIME 9시간 차이 확인
-- SELECT TOP 5
--   t.SEGMENT_ID,
--   t.START_TIME    AS UTC,
--   v.START_TIME    AS KST,
--   DATEDIFF(hour, t.START_TIME, v.START_TIME) AS DIFF_HOURS
-- FROM dbo.PCAGT_IDLE_SEGMENT t
-- JOIN dbo.v_PCAGT_IDLE_SEGMENT_KST v ON t.SEGMENT_ID = v.SEGMENT_ID
-- ORDER BY t.REG_DT DESC;
--   → 모든 DIFF_HOURS = 9 여야 정상
