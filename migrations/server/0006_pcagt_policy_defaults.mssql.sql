-- ============================================================================
-- PCAGT_POLICY 컬럼 DEFAULT 제약 + NULL 백필 (티켓 T-20260512-01 P0)
-- ----------------------------------------------------------------------------
-- 배경: PATCH /policy 부분 업데이트 또는 운영자 DB 직접 조작 시 NULL 사고 차단.
--       GET /policy 자동 INSERT 로직과 짝이 됨 (서비스 코드는 minimal 컬럼만
--       INSERT 하고 나머지는 DEFAULT 가 채움).
--
-- 적용 대상 컬럼:
--   IDLE_THRESHOLD_SECONDS       DEFAULT 600
--   LUNCH_START_TIME             DEFAULT '11:30'
--   LUNCH_END_TIME               DEFAULT '14:00'
--   LUNCH_ALLOWED_MINUTES        DEFAULT 60
--   EXPLANATION_DEADLINE_HOURS   DEFAULT 48
--   CAN_TRACK_TIME               이미 DEFAULT(1) 있음 — 확인만
--
-- 0005 에서 DROP 된 HEARTBEAT_INTERVAL_SECONDS / EVENT_BATCH_INTERVAL_SECONDS 는
-- 대상 아님 (컬럼 자체가 없음).
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) DEFAULT 제약 추가 (없을 때만)
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.default_constraints WHERE name = N'DF_PCAGT_POL_IDLE')
    ALTER TABLE dbo.PCAGT_POLICY
      ADD CONSTRAINT DF_PCAGT_POL_IDLE DEFAULT (600) FOR IDLE_THRESHOLD_SECONDS;
GO

IF NOT EXISTS (SELECT 1 FROM sys.default_constraints WHERE name = N'DF_PCAGT_POL_LS')
    ALTER TABLE dbo.PCAGT_POLICY
      ADD CONSTRAINT DF_PCAGT_POL_LS DEFAULT ('11:30') FOR LUNCH_START_TIME;
GO

IF NOT EXISTS (SELECT 1 FROM sys.default_constraints WHERE name = N'DF_PCAGT_POL_LE')
    ALTER TABLE dbo.PCAGT_POLICY
      ADD CONSTRAINT DF_PCAGT_POL_LE DEFAULT ('14:00') FOR LUNCH_END_TIME;
GO

IF NOT EXISTS (SELECT 1 FROM sys.default_constraints WHERE name = N'DF_PCAGT_POL_LM')
    ALTER TABLE dbo.PCAGT_POLICY
      ADD CONSTRAINT DF_PCAGT_POL_LM DEFAULT (60) FOR LUNCH_ALLOWED_MINUTES;
GO

IF NOT EXISTS (SELECT 1 FROM sys.default_constraints WHERE name = N'DF_PCAGT_POL_ED')
    ALTER TABLE dbo.PCAGT_POLICY
      ADD CONSTRAINT DF_PCAGT_POL_ED DEFAULT (48) FOR EXPLANATION_DEADLINE_HOURS;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) 기존 row 의 NULL 백필
--    DEFAULT 제약은 신규 INSERT 에만 적용되므로 기존 row 는 별도 UPDATE.
-- ────────────────────────────────────────────────────────────────────────────
UPDATE dbo.PCAGT_POLICY
   SET IDLE_THRESHOLD_SECONDS = 600
 WHERE IDLE_THRESHOLD_SECONDS IS NULL;
GO

UPDATE dbo.PCAGT_POLICY
   SET LUNCH_START_TIME = '11:30'
 WHERE LUNCH_START_TIME IS NULL;
GO

UPDATE dbo.PCAGT_POLICY
   SET LUNCH_END_TIME = '14:00'
 WHERE LUNCH_END_TIME IS NULL;
GO

UPDATE dbo.PCAGT_POLICY
   SET LUNCH_ALLOWED_MINUTES = 60
 WHERE LUNCH_ALLOWED_MINUTES IS NULL;
GO

UPDATE dbo.PCAGT_POLICY
   SET EXPLANATION_DEADLINE_HOURS = 48
 WHERE EXPLANATION_DEADLINE_HOURS IS NULL;
GO
