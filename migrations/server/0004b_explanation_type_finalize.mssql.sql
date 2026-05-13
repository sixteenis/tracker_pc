-- ============================================================================
-- 0004b — PCAGT_EXPLANATION_TYPE 마무리 (인덱스 누락 + 중복 정리 + 시드 보충)
-- ----------------------------------------------------------------------------
-- 배경:
--   0004 마이그레이션 첫 실행 시 sqlcmd 가 QUOTED_IDENTIFIER OFF 인 상태에서
--   CREATE UNIQUE INDEX 가 실패 → BEGIN 블록 abort. 두 번째 실행(-I 추가)
--   시점에는 IF NOT EXISTS 가드로 BEGIN 블록 전체 skip → 인덱스가 영영 안 생김.
--
--   결과: UNIQUE 인덱스 부재로 (CMPSID, CODE) 중복 INSERT 허용됨. 시연 도중
--   cmpsid=11402 의 'TEST' code 가 3 row 활성 상태로 발견. 시스템 시드 12개
--   도 자동 시드 로직(0건일 때만 동작) 이 4 row 존재로 인해 트리거 안 됨.
--
-- 이 마이그레이션이 하는 일 (idempotent):
--   1) 중복 활성 row 정리 — 같은 (CMPSID, CODE) 활성 row 가 2건 이상이면
--      가장 큰 EXPTYPE_SID 만 유지, 나머지는 IS_ACTIVE=0 으로 비활성화.
--   2) UNIQUE 필터 인덱스 3개 (COMPANY/TOPTEAM/TEAM) + 일반 필터 인덱스 3개
--      (IF NOT EXISTS 가드).
--   3) cmpsid=11402 회사에 시스템 시드 12개 보충 — 활성 동일 코드 없을 때만
--      INSERT (NOT EXISTS 가드). 다른 회사는 자동 시드 로직이 처리하므로 손대지 않음.
--
-- 적용 명령:
--   docker exec -i mssql_server /opt/mssql-tools18/bin/sqlcmd \
--     -S 192.168.0.181 -U sa -P 'wry_app_2024**' -d wooriyo0n02 -C -I -b \
--     < ~/Desktop/psm/psm_AOS/pinple_pc_agent/migrations/server/0004b_explanation_type_finalize.mssql.sql
-- ============================================================================

SET NOCOUNT ON;
SET QUOTED_IDENTIFIER ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) 중복 활성 row 정리
--    같은 (SCOPE, CMPSID, TTMSID, TEMSID, CODE) 활성 row 가 2건 이상일 때
--    가장 큰 EXPTYPE_SID 만 유지, 나머지는 IS_ACTIVE=0.
-- ────────────────────────────────────────────────────────────────────────────
;WITH ranked AS (
    SELECT
        EXPTYPE_SID,
        ROW_NUMBER() OVER (
            PARTITION BY SCOPE, CMPSID,
                ISNULL(TTMSID, -1), ISNULL(TEMSID, -1), CODE
            ORDER BY EXPTYPE_SID DESC
        ) AS rn
    FROM dbo.PCAGT_EXPLANATION_TYPE
    WHERE IS_ACTIVE = 1
)
UPDATE t
   SET IS_ACTIVE = 0,
       UPD_DT = SYSUTCDATETIME()
  FROM dbo.PCAGT_EXPLANATION_TYPE t
  JOIN ranked r ON r.EXPTYPE_SID = t.EXPTYPE_SID
 WHERE r.rn > 1;
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) UNIQUE 필터 인덱스 3개 + 일반 필터 인덱스 3개
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.indexes
                WHERE name = N'UX_PCAGT_EXPTYPE_CMP_CODE_ACT'
                  AND object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
    CREATE UNIQUE INDEX UX_PCAGT_EXPTYPE_CMP_CODE_ACT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, CODE)
        WHERE IS_ACTIVE = 1 AND SCOPE = 'COMPANY';
GO

IF NOT EXISTS (SELECT 1 FROM sys.indexes
                WHERE name = N'UX_PCAGT_EXPTYPE_TT_CODE_ACT'
                  AND object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
    CREATE UNIQUE INDEX UX_PCAGT_EXPTYPE_TT_CODE_ACT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, CODE)
        WHERE IS_ACTIVE = 1 AND SCOPE = 'TOPTEAM';
GO

IF NOT EXISTS (SELECT 1 FROM sys.indexes
                WHERE name = N'UX_PCAGT_EXPTYPE_TM_CODE_ACT'
                  AND object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
    CREATE UNIQUE INDEX UX_PCAGT_EXPTYPE_TM_CODE_ACT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, TEMSID, CODE)
        WHERE IS_ACTIVE = 1 AND SCOPE = 'TEAM';
GO

IF NOT EXISTS (SELECT 1 FROM sys.indexes
                WHERE name = N'IX_PCAGT_EXPTYPE_CMP'
                  AND object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
    CREATE INDEX IX_PCAGT_EXPTYPE_CMP
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, IS_ACTIVE)
        WHERE SCOPE = 'COMPANY';
GO

IF NOT EXISTS (SELECT 1 FROM sys.indexes
                WHERE name = N'IX_PCAGT_EXPTYPE_TT'
                  AND object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
    CREATE INDEX IX_PCAGT_EXPTYPE_TT
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, IS_ACTIVE)
        WHERE SCOPE = 'TOPTEAM';
GO

IF NOT EXISTS (SELECT 1 FROM sys.indexes
                WHERE name = N'IX_PCAGT_EXPTYPE_TM'
                  AND object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
    CREATE INDEX IX_PCAGT_EXPTYPE_TM
        ON dbo.PCAGT_EXPLANATION_TYPE (CMPSID, TTMSID, TEMSID, IS_ACTIVE)
        WHERE SCOPE = 'TEAM';
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 3) cmpsid=11402 시스템 시드 12개 보충
--    같은 회사·코드 활성 row 가 이미 있으면 INSERT skip (NOT EXISTS 가드).
--    시드는 시스템 기본 — IS_SYSTEM=1, IS_ACTIVE=1.
-- ────────────────────────────────────────────────────────────────────────────
INSERT INTO dbo.PCAGT_EXPLANATION_TYPE
        (SCOPE, CMPSID, CODE, LABEL, SORT_ORDER, REQUIRES_TEXT, IS_SYSTEM, IS_ACTIVE)
SELECT v.SCOPE, v.CMPSID, v.CODE, v.LABEL, v.SORT_ORDER, v.REQUIRES_TEXT, 1, 1
  FROM (VALUES
        ('COMPANY', 11402, 'MEETING',           N'회의',      10,  0),
        ('COMPANY', 11402, 'PHONE_CALL',        N'전화상담',  20,  0),
        ('COMPANY', 11402, 'CUSTOMER_RESPONSE', N'고객대응',  30,  0),
        ('COMPANY', 11402, 'BUSINESS_TRIP',     N'출장',      40,  0),
        ('COMPANY', 11402, 'OUTSIDE_WORK',      N'외근',      50,  1),
        ('COMPANY', 11402, 'EDUCATION',         N'교육',      60,  0),
        ('COMPANY', 11402, 'WORK_WAITING',      N'업무 대기', 70,  0),
        ('COMPANY', 11402, 'PC_ERROR',          N'PC 오류',   80,  0),
        ('COMPANY', 11402, 'APP_ERROR',         N'앱 오류',   90,  0),
        ('COMPANY', 11402, 'OTHER_WORK',        N'기타 업무', 100, 1),
        ('COMPANY', 11402, 'LUNCH_BREAK',       N'점심시간',  110, 0),
        ('COMPANY', 11402, 'PERSONAL',          N'개인 사유', 120, 1)
       ) AS v(SCOPE, CMPSID, CODE, LABEL, SORT_ORDER, REQUIRES_TEXT)
 WHERE NOT EXISTS (
        SELECT 1 FROM dbo.PCAGT_EXPLANATION_TYPE e
         WHERE e.SCOPE = v.SCOPE
           AND e.CMPSID = v.CMPSID
           AND e.CODE = v.CODE
           AND e.IS_ACTIVE = 1
       );
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 4) 검증 SELECT — 적용 후 상태 출력
-- ────────────────────────────────────────────────────────────────────────────
PRINT '--- 인덱스 (UX*, IX_PCAGT_EXPTYPE_*) ---';
SELECT name FROM sys.indexes
 WHERE object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE')
   AND name IS NOT NULL
 ORDER BY name;

PRINT '--- cmpsid=11402 활성 row ---';
SELECT EXPTYPE_SID, CODE, LABEL, IS_SYSTEM, IS_ACTIVE, SORT_ORDER
  FROM dbo.PCAGT_EXPLANATION_TYPE
 WHERE CMPSID = 11402 AND IS_ACTIVE = 1
 ORDER BY SORT_ORDER, EXPTYPE_SID;
GO
