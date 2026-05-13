-- ============================================================================
-- 핀플 PC Agent V2 — 소명사유 회사 커스텀 (MVP: COMPANY 스코프)
-- ----------------------------------------------------------------------------
-- 배경: PCAGT_EXPLANATION.EXPLANATION_TYPE 가 12개 enum 하드코딩이라 회사별
--       커스터마이즈 불가. 룩업 테이블 PCAGT_EXPLANATION_TYPE 도입.
--
-- 스코프: SCOPE = 'COMPANY' / 'TOPTEAM' / 'TEAM'
--   - MVP 는 'COMPANY' 만 사용. 'TOPTEAM' / 'TEAM' 은 Phase 3 활성화 예정이라
--     컬럼·CHECK 미리 포함해서 향후 ALTER TABLE 회피.
--   - 서비스 레이어가 MVP 동안 SCOPE='COMPANY' 만 처리.
--
-- 자동 시드: 회사 첫 조회 시 application 레벨에서 시스템 기본 12개를
--   (SCOPE='COMPANY', CMPSID=회사ID, IS_SYSTEM=1, IS_ACTIVE=1) 로 INSERT.
--
-- 외래키: 없음 (기존 무손상 정책). PCAGT_EXPLANATION.EXPLANATION_TYPE 도
--   문자열 그대로 보관. 검증은 서비스 레이어가 lookup 으로 수행.
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) PCAGT_EXPLANATION_TYPE 신설
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_EXPLANATION_TYPE (
        EXPTYPE_SID    BIGINT          IDENTITY(1,1) NOT NULL,

        SCOPE          NVARCHAR(20)    NOT NULL,        -- 'COMPANY'/'TOPTEAM'/'TEAM'
        CMPSID         INT             NOT NULL,        -- 모든 스코프에서 필수
        TTMSID         INT             NULL,            -- 'TOPTEAM','TEAM' 스코프에서 필수
        TEMSID         INT             NULL,            -- 'TEAM' 스코프에서 필수

        CODE           NVARCHAR(40)    NOT NULL,        -- API/DB 키 (예: MEETING)
        LABEL          NVARCHAR(100)   NOT NULL,        -- UI 표시명 (한국어)
        SORT_ORDER     INT             NOT NULL CONSTRAINT DF_PCAGT_EXPTYPE_SORT DEFAULT(100),
        ICON           NVARCHAR(40)    NULL,
        REQUIRES_TEXT  BIT             NOT NULL CONSTRAINT DF_PCAGT_EXPTYPE_RT   DEFAULT(0),
        IS_SYSTEM      BIT             NOT NULL CONSTRAINT DF_PCAGT_EXPTYPE_SYS  DEFAULT(0),
        IS_ACTIVE      BIT             NOT NULL CONSTRAINT DF_PCAGT_EXPTYPE_ACT  DEFAULT(1),

        REG_DT         DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_EXPTYPE_REG  DEFAULT(SYSUTCDATETIME()),
        UPD_DT         DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_EXPTYPE_UPD  DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_EXPLANATION_TYPE PRIMARY KEY CLUSTERED (EXPTYPE_SID),
        CONSTRAINT CK_PCAGT_EXPTYPE_SCOPE CHECK (SCOPE IN ('COMPANY','TOPTEAM','TEAM')),
        CONSTRAINT CK_PCAGT_EXPTYPE_KEYS CHECK (
            (SCOPE = 'COMPANY' AND TTMSID IS NULL     AND TEMSID IS NULL)
         OR (SCOPE = 'TOPTEAM' AND TTMSID IS NOT NULL AND TEMSID IS NULL)
         OR (SCOPE = 'TEAM'    AND TTMSID IS NOT NULL AND TEMSID IS NOT NULL)
        )
    );

    -- 활성 row 의 CODE 는 각 스코프에서 유일
    CREATE UNIQUE INDEX UX_PCAGT_EXPTYPE_CMP_CODE_ACT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, CODE)
        WHERE IS_ACTIVE = 1 AND SCOPE = 'COMPANY';

    CREATE UNIQUE INDEX UX_PCAGT_EXPTYPE_TT_CODE_ACT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, CODE)
        WHERE IS_ACTIVE = 1 AND SCOPE = 'TOPTEAM';

    CREATE UNIQUE INDEX UX_PCAGT_EXPTYPE_TM_CODE_ACT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, TEMSID, CODE)
        WHERE IS_ACTIVE = 1 AND SCOPE = 'TEAM';

    -- 스코프별 조회 인덱스
    CREATE INDEX IX_PCAGT_EXPTYPE_CMP
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, IS_ACTIVE)
        WHERE SCOPE = 'COMPANY';

    CREATE INDEX IX_PCAGT_EXPTYPE_TT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, IS_ACTIVE)
        WHERE SCOPE = 'TOPTEAM';

    CREATE INDEX IX_PCAGT_EXPTYPE_TM
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, TEMSID, IS_ACTIVE)
        WHERE SCOPE = 'TEAM';
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) 기존 PCAGT_EXPLANATION.EXPLANATION_TYPE CHECK 제거
--    이제 검증은 서비스 레이어가 PCAGT_EXPLANATION_TYPE 룩업으로 수행한다.
--    회사가 사유를 추가하면 새 코드도 INSERT 가능해진다.
-- ────────────────────────────────────────────────────────────────────────────
IF EXISTS (SELECT 1 FROM sys.check_constraints WHERE name = N'CK_PCAGT_EXP_TYPE')
    ALTER TABLE dbo.PCAGT_EXPLANATION DROP CONSTRAINT CK_PCAGT_EXP_TYPE;
GO
