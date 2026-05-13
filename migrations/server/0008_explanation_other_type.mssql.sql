-- ============================================================================
-- 핀플 PC Agent V2 — "기타" 소명사유 + 보호 시스템 row + 자유 입력 라벨
-- ----------------------------------------------------------------------------
-- 기획 변경(2026-05-13): 사용자가 회사 사유 목록에 없는 사유를 즉시 입력할 수
--   있도록 "기타"(code='OTHER') 사유를 도입. 관리자는 비활성화 불가(보호).
--   "기타" 선택 시 클라가 1~50자 자유 라벨을 보내 PCAGT_EXPLANATION.OTHER_TYPE_LABEL
--   컬럼에 저장. 다른 사유 row 에서는 NULL.
--
-- 변경:
--   1) PCAGT_EXPLANATION         + OTHER_TYPE_LABEL NVARCHAR(50) NULL
--   2) PCAGT_EXPLANATION_TYPE    + IS_PROTECTED     BIT          NOT NULL DEFAULT 0
--   3) 기존 회사 row 들에 'OTHER' 시드 백필 (IS_SYSTEM=1, IS_PROTECTED=1)
--
-- 호환:
--   - 기존 row 의 OTHER_TYPE_LABEL = NULL (영향 없음)
--   - IS_PROTECTED DEFAULT 0 → 기존 시스템 시드 12개는 IS_PROTECTED=0 (deactivate 가능 유지)
--   - 신규 'OTHER' 시드만 IS_PROTECTED=1
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) PCAGT_EXPLANATION.OTHER_TYPE_LABEL (기타 선택 시 사용자 입력 라벨)
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (
    SELECT 1 FROM sys.columns
     WHERE object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION')
       AND name      = N'OTHER_TYPE_LABEL'
)
BEGIN
    ALTER TABLE dbo.PCAGT_EXPLANATION
        ADD OTHER_TYPE_LABEL NVARCHAR(50) NULL;
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2) PCAGT_EXPLANATION_TYPE.IS_PROTECTED (1 → CMS deactivate 거부)
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (
    SELECT 1 FROM sys.columns
     WHERE object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE')
       AND name      = N'IS_PROTECTED'
)
BEGIN
    ALTER TABLE dbo.PCAGT_EXPLANATION_TYPE
        ADD IS_PROTECTED BIT NOT NULL
            CONSTRAINT DF_PCAGT_EXPTYPE_PROT DEFAULT(0);
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 3) 기존 회사 row 들에 'OTHER' 시드 백필
--    UX_PCAGT_EXPTYPE_CMP_CODE_ACT 가 (CMPSID, CODE) 활성 중복을 차단하므로
--    NOT EXISTS 가드로 멱등 수행. 이미 'OTHER' 가 있는 회사는 건너뜀.
-- ────────────────────────────────────────────────────────────────────────────
INSERT INTO dbo.PCAGT_EXPLANATION_TYPE
    (SCOPE, CMPSID, CODE, LABEL, SORT_ORDER, REQUIRES_TEXT, IS_SYSTEM, IS_PROTECTED, IS_ACTIVE)
SELECT DISTINCT
    'COMPANY', t.CMPSID, 'OTHER', N'기타', 999, 0, 1, 1, 1
  FROM dbo.PCAGT_EXPLANATION_TYPE t
 WHERE t.SCOPE = 'COMPANY'
   AND NOT EXISTS (
       SELECT 1
         FROM dbo.PCAGT_EXPLANATION_TYPE x
        WHERE x.SCOPE  = 'COMPANY'
          AND x.CMPSID = t.CMPSID
          AND x.CODE   = 'OTHER'
   );
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 검증 쿼리 (실행 후 확인)
-- ────────────────────────────────────────────────────────────────────────────
-- 1) 컬럼 추가 확인
-- SELECT name, system_type_id, max_length, is_nullable
--   FROM sys.columns
--  WHERE object_id IN (OBJECT_ID(N'dbo.PCAGT_EXPLANATION'), OBJECT_ID(N'dbo.PCAGT_EXPLANATION_TYPE'))
--    AND name IN ('OTHER_TYPE_LABEL', 'IS_PROTECTED');
--
-- 2) 'OTHER' 시드 백필 확인 (회사별로 1건씩)
-- SELECT CMPSID, COUNT(*) AS OTHER_CNT
--   FROM dbo.PCAGT_EXPLANATION_TYPE
--  WHERE SCOPE = 'COMPANY' AND CODE = 'OTHER' AND IS_ACTIVE = 1
--  GROUP BY CMPSID;
