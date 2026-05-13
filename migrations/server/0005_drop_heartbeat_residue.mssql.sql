-- ============================================================================
-- heartbeat 제거 잔여 정리 (정책 컬럼 / EVENT_TYPE CHECK)
-- ----------------------------------------------------------------------------
-- 배경: heartbeat API 폐기([[변경_heartbeat_제거_및_PRESENCE_DB]]) 진행 중.
--       클라이언트는 이미 heartbeat 호출 중단(§12) 했으므로 아래 잔재 제거.
--
-- 이 마이그레이션이 다루는 것:
--   1) PCAGT_POLICY 의 HEARTBEAT_INTERVAL_SECONDS, EVENT_BATCH_INTERVAL_SECONDS
--      컬럼 제거. /policy 응답에서 두 필드 사라지므로 클라가 더 이상 못 가져옴.
--   2) PCAGT_EVENT.CK_PCAGT_EVT_TYPE 재생성 — 'HEARTBEAT' 제거 + 'LOGIN_SUCCESS'/
--      'LOGOUT' 추가 (heartbeat 변경 문서 §13.3.2 P0).
--      기존 HEARTBEAT row 가 있을 수 있어 WITH NOCHECK 사용 (신규 INSERT 만 가드).
--
-- 이 마이그레이션이 다루지 않는 것 (변경_heartbeat 문서 Step 4 별도):
--   - PCAGT_HEARTBEAT 테이블 DROP
--   - PCAGT_DEVICE_SESSION.LAST_HEARTBEAT_AT 컬럼 DROP
--   → destructive 라 별도 마이그레이션 + 시점 결정 필요.
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) PCAGT_POLICY 컬럼 제거
-- ────────────────────────────────────────────────────────────────────────────
IF EXISTS (SELECT 1 FROM sys.columns
            WHERE name = N'HEARTBEAT_INTERVAL_SECONDS'
              AND object_id = OBJECT_ID(N'dbo.PCAGT_POLICY'))
    ALTER TABLE dbo.PCAGT_POLICY DROP COLUMN HEARTBEAT_INTERVAL_SECONDS;
GO

IF EXISTS (SELECT 1 FROM sys.columns
            WHERE name = N'EVENT_BATCH_INTERVAL_SECONDS'
              AND object_id = OBJECT_ID(N'dbo.PCAGT_POLICY'))
    ALTER TABLE dbo.PCAGT_POLICY DROP COLUMN EVENT_BATCH_INTERVAL_SECONDS;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) PCAGT_EVENT.EVENT_TYPE CHECK 재생성
--    - 제거: 'HEARTBEAT'        (이벤트 채널에서 heartbeat 더 이상 송신 안 함)
--    - 추가: 'LOGIN_SUCCESS'    (수동 로그인, PRESENCE 매핑 대상)
--    - 추가: 'LOGOUT'           (USER_ACTION / FORCE_LOGOUT reason 구분)
--    기존 'HEARTBEAT' row 는 그대로 두기 위해 WITH NOCHECK (감사 데이터 무손실).
-- ────────────────────────────────────────────────────────────────────────────
IF EXISTS (SELECT 1 FROM sys.check_constraints WHERE name = N'CK_PCAGT_EVT_TYPE')
    ALTER TABLE dbo.PCAGT_EVENT DROP CONSTRAINT CK_PCAGT_EVT_TYPE;
GO

ALTER TABLE dbo.PCAGT_EVENT WITH NOCHECK
  ADD CONSTRAINT CK_PCAGT_EVT_TYPE CHECK (EVENT_TYPE IN (
    'APP_STARTED','APP_STOPPED','AUTO_LOGIN_SUCCESS','AUTO_LOGIN_FAILED',
    'LOGIN_SUCCESS','LOGOUT',
    'USER_ACTIVE','IDLE_STARTED','IDLE_ENDED','PC_LOCKED','PC_UNLOCKED',
    'PC_SHUTDOWN_DETECTED','NO_PC_RECORD','EXPLANATION_SUBMITTED',
    'SYNC_SUCCESS','SYNC_FAILED'
  ));
GO
