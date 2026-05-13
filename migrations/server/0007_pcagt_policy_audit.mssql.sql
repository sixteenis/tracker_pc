-- ============================================================================
-- PCAGT_POLICY_AUDIT 신설 (티켓 T-20260512-01 P1, 기획자 결정 2026-05-12)
-- ----------------------------------------------------------------------------
-- 배경: PATCH /policy 로 변경된 필드별 이력 보관. 운영자 누가/언제/무엇을/왜
--       바꿨는지 감사. 보관 3년(1095일).
--
-- 운영 규칙:
--   - PATCH 시 변경된 필드마다 1 row INSERT (한 PATCH 가 3개 필드 변경 시 3 row).
--   - OLD_VALUE / NEW_VALUE 는 모두 NVARCHAR 로 정규화 저장.
--   - 변경자 EMPSID 는 PATCH 요청자(회사 관리자) 식별자.
-- ============================================================================

SET NOCOUNT ON;
GO

IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_POLICY_AUDIT') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_POLICY_AUDIT (
        AUDIT_SID     BIGINT          IDENTITY(1,1) NOT NULL,
        CMPSID        INT             NOT NULL,
        EMPSID        INT             NOT NULL,            -- 변경자
        CHANGED_AT    DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_POLAUD_CH  DEFAULT(SYSUTCDATETIME()),
        FIELD_NAME    NVARCHAR(50)    NOT NULL,            -- 'IDLE_THRESHOLD_SECONDS' 등
        OLD_VALUE     NVARCHAR(100)   NULL,                -- 정규화된 문자열 (NULL 가능)
        NEW_VALUE     NVARCHAR(100)   NULL,                -- 정규화된 문자열 (NULL 가능)
        REASON        NVARCHAR(500)   NULL,                -- 운영자 메모 (옵션)
        REG_DT        DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_POLAUD_REG DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_POLICY_AUDIT PRIMARY KEY CLUSTERED (AUDIT_SID)
    );

    CREATE INDEX IX_PCAGT_POLAUD_CMP_TIME ON dbo.PCAGT_POLICY_AUDIT (CMPSID, CHANGED_AT DESC);
    CREATE INDEX IX_PCAGT_POLAUD_EMP_TIME ON dbo.PCAGT_POLICY_AUDIT (EMPSID, CHANGED_AT DESC);
END
GO
