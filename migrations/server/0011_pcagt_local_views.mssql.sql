-- ============================================================================
-- 핀플 PC Agent V2 — 운영자용 회사 timezone 뷰 (2026-05-13)
-- ----------------------------------------------------------------------------
-- 배경:
--   0009 에서 `v_PCAGT_*_KST` 뷰 (고정 +9 KST) 신설했으나, 0010 에서
--   회사별 `TIME_ZONE_OFFSET_MINUTES` 컬럼 도입. 이제 회사 timezone 에 맞춰
--   변환하는 뷰가 필요.
--
-- 변경:
--   - 기존 `v_PCAGT_*_KST` 10개 DROP (이름 미스리딩 — 더이상 KST 고정 아님)
--   - 신규 `v_PCAGT_*_LOCAL` 10개 — 회사 COMPANY 스코프 활성 정책의
--     TIME_ZONE_OFFSET_MINUTES 로 변환
--
-- 동작 규칙:
--   - 회사 정책 row 가 없거나 NULL 이면 540 (KST) fallback
--   - LEFT JOIN 으로 회사 정책 가져옴 (없어도 row 누락 안 됨)
--   - 자기 자신 회사 정책 참조 (`PCAGT_POLICY` 뷰는 self join)
--   - 회사 컬럼 없는 테이블 (`PCAGT_APP_VERSION`): 540 고정
--
-- 운영자 사용:
--   SELECT * FROM dbo.v_PCAGT_IDLE_SEGMENT_LOCAL WHERE EMPSID = 48660;
--   → 해당 회사 timezone 으로 자동 변환된 시각 표시
--
-- 멱등성: `CREATE OR ALTER VIEW`. 기존 _KST 뷰는 명시 DROP.
-- ============================================================================

SET NOCOUNT ON;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 0) 기존 _KST 뷰 정리 (이름 미스리딩 회피)
-- ────────────────────────────────────────────────────────────────────────────
IF OBJECT_ID('dbo.v_PCAGT_DEVICE_SESSION_KST',     'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_DEVICE_SESSION_KST;
IF OBJECT_ID('dbo.v_PCAGT_POLICY_KST',             'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_POLICY_KST;
IF OBJECT_ID('dbo.v_PCAGT_APP_VERSION_KST',        'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_APP_VERSION_KST;
IF OBJECT_ID('dbo.v_PCAGT_HEARTBEAT_KST',          'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_HEARTBEAT_KST;
IF OBJECT_ID('dbo.v_PCAGT_EVENT_KST',              'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_EVENT_KST;
IF OBJECT_ID('dbo.v_PCAGT_IDLE_SEGMENT_KST',       'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_IDLE_SEGMENT_KST;
IF OBJECT_ID('dbo.v_PCAGT_EXPLANATION_KST',        'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_EXPLANATION_KST;
IF OBJECT_ID('dbo.v_PCAGT_ATTENDANCE_SNAPSHOT_KST','V') IS NOT NULL DROP VIEW dbo.v_PCAGT_ATTENDANCE_SNAPSHOT_KST;
IF OBJECT_ID('dbo.v_PCAGT_EXPLANATION_TYPE_KST',   'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_EXPLANATION_TYPE_KST;
IF OBJECT_ID('dbo.v_PCAGT_POLICY_AUDIT_KST',       'V') IS NOT NULL DROP VIEW dbo.v_PCAGT_POLICY_AUDIT_KST;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 1) v_PCAGT_DEVICE_SESSION_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_DEVICE_SESSION_LOCAL AS
SELECT
    t.SESSION_SID, t.CMPSID, t.MBRSID, t.EMPSID, t.TTMSID, t.TEMSID,
    t.DEVICE_ID, t.DEVICE_NAME, t.OS, t.APP_VERSION, t.IS_ACTIVE,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.DISPLACED_AT)      AS DISPLACED_AT,
    t.DISPLACED_REASON,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.LAST_LOGIN_AT)     AS LAST_LOGIN_AT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.LAST_HEARTBEAT_AT) AS LAST_HEARTBEAT_AT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT)            AS REG_DT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.UPD_DT)            AS UPD_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_DEVICE_SESSION t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) v_PCAGT_POLICY_LOCAL (self-ref)
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_POLICY_LOCAL AS
SELECT
    t.POLICY_SID, t.POLICY_SCOPE, t.CMPSID, t.TTMSID, t.TEMSID, t.EMPSID,
    t.IDLE_THRESHOLD_SECONDS,
    t.LUNCH_START_TIME, t.LUNCH_END_TIME, t.LUNCH_ALLOWED_MINUTES,
    t.EXPLANATION_DEADLINE_HOURS,
    t.CAN_TRACK_TIME, t.POLICY_VERSION, t.IS_ACTIVE,
    t.TIME_ZONE_OFFSET_MINUTES,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT) AS REG_DT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.UPD_DT) AS UPD_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_POLICY t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 3) v_PCAGT_APP_VERSION_LOCAL (회사 컬럼 없음 — 540 고정)
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_APP_VERSION_LOCAL AS
SELECT
    APPVER_SID, OS, LATEST_VERSION, MINIMUM_REQUIRED_VERSION,
    FORCE_UPDATE, DOWNLOAD_URL, RELEASE_NOTE, IS_ACTIVE,
    DATEADD(minute, 540, REG_DT) AS REG_DT,
    DATEADD(minute, 540, UPD_DT) AS UPD_DT,
    540 AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_APP_VERSION;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 4) v_PCAGT_HEARTBEAT_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_HEARTBEAT_LOCAL AS
SELECT
    t.HB_SID, t.CMPSID, t.EMPSID, t.DEVICE_ID, t.DEVICE_NAME, t.APP_VERSION,
    t.PC_STATUS,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.LAST_ACTIVITY_AT) AS LAST_ACTIVITY_AT,
    t.IDLE_SECONDS, t.IS_LOCKED, t.ATTENDANCE_STATUS, t.CAN_TRACK_TIME,
    t.EFFECTIVE_IDLE_THRESHOLD_SECONDS,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT) AS REG_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_HEARTBEAT t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 5) v_PCAGT_EVENT_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_EVENT_LOCAL AS
SELECT
    t.EVENT_SID, t.EVENT_ID, t.CMPSID, t.EMPSID, t.DEVICE_ID,
    t.EVENT_TYPE,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.EVENT_TIME)  AS EVENT_TIME,
    t.PAYLOAD_JSON,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.RECEIVED_AT) AS RECEIVED_AT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_EVENT t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 6) v_PCAGT_IDLE_SEGMENT_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_IDLE_SEGMENT_LOCAL AS
SELECT
    t.SEGMENT_ID, t.CMPSID, t.EMPSID, t.DEVICE_ID,
    t.WORK_DATE,                                   -- DATE — 사용자 로컬 가정 그대로
    t.SEGMENT_TYPE,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.START_TIME)           AS START_TIME,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.END_TIME)             AS END_TIME,
    t.DURATION_SECONDS,
    t.APPLIED_IDLE_THRESHOLD_SECONDS, t.POLICY_SCOPE,
    t.EXPLANATION_REQUIRED,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.EXPLANATION_DEADLINE) AS EXPLANATION_DEADLINE,
    t.EXPLANATION_STATUS, t.WORKTIME_REFLECTION_STATUS,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT)               AS REG_DT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.UPD_DT)               AS UPD_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_IDLE_SEGMENT t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 7) v_PCAGT_EXPLANATION_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_EXPLANATION_LOCAL AS
SELECT
    t.EXP_SID, t.SEGMENT_ID, t.CMPSID, t.EMPSID,
    t.EXPLANATION_TYPE, t.EXPLANATION_TEXT, t.OTHER_TYPE_LABEL, t.SUBMITTED_FROM,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.SUBMITTED_AT) AS SUBMITTED_AT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT)       AS REG_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_EXPLANATION t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 8) v_PCAGT_ATTENDANCE_SNAPSHOT_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_ATTENDANCE_SNAPSHOT_LOCAL AS
SELECT
    t.ATT_SID, t.CMPSID, t.EMPSID,
    t.WORK_DATE,                              -- DATE — 사용자 로컬 가정 그대로
    t.ATTENDANCE_STATUS,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.WORK_START_AT) AS WORK_START_AT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.WORK_END_AT)   AS WORK_END_AT,
    t.SOURCE,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT)        AS REG_DT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.UPD_DT)        AS UPD_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_ATTENDANCE_SNAPSHOT t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 9) v_PCAGT_EXPLANATION_TYPE_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_EXPLANATION_TYPE_LOCAL AS
SELECT
    t.EXPTYPE_SID, t.SCOPE, t.CMPSID, t.TTMSID, t.TEMSID,
    t.CODE, t.LABEL, t.SORT_ORDER, t.ICON,
    t.REQUIRES_TEXT, t.IS_SYSTEM, t.IS_PROTECTED, t.IS_ACTIVE,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT) AS REG_DT,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.UPD_DT) AS UPD_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_EXPLANATION_TYPE t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 10) v_PCAGT_POLICY_AUDIT_LOCAL
-- ────────────────────────────────────────────────────────────────────────────
CREATE OR ALTER VIEW dbo.v_PCAGT_POLICY_AUDIT_LOCAL AS
SELECT
    t.AUDIT_SID, t.CMPSID, t.EMPSID,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.CHANGED_AT) AS CHANGED_AT,
    t.FIELD_NAME, t.OLD_VALUE, t.NEW_VALUE, t.REASON,
    DATEADD(minute, COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540), t.REG_DT)     AS REG_DT,
    COALESCE(p.TIME_ZONE_OFFSET_MINUTES, 540) AS APPLIED_TZ_OFFSET_MINUTES
FROM dbo.PCAGT_POLICY_AUDIT t
LEFT JOIN dbo.PCAGT_POLICY p
  ON p.CMPSID = t.CMPSID AND p.POLICY_SCOPE = 'COMPANY' AND p.IS_ACTIVE = 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 검증 쿼리
-- ────────────────────────────────────────────────────────────────────────────
-- 1) 10개 _LOCAL 뷰 + 0개 _KST 뷰 (모두 DROP 됐는지)
-- SELECT name FROM sys.views WHERE name LIKE 'v_PCAGT[_]%' ORDER BY name;
--
-- 2) 회사 timezone 적용 확인
-- SELECT TOP 3 SEGMENT_ID, START_TIME, APPLIED_TZ_OFFSET_MINUTES
--   FROM dbo.v_PCAGT_IDLE_SEGMENT_LOCAL ORDER BY REG_DT DESC;
