-- ============================================================================
-- 핀플 PC Agent V2 — 회사별 timezone offset 설정 (2026-05-13)
-- ----------------------------------------------------------------------------
-- 배경:
--   서버는 모든 시각을 UTC 로 저장하고 클라/UI 가 KST 로 변환해 표시 중.
--   추후 회사 지사가 다른 국가(예: 미국 동부 EST -300m)에 있을 수 있어,
--   회사별 timezone 을 정책에 두고 운영자/관리자가 변경 가능하게 한다.
--
-- 결정 (2026-05-13):
--   - MVP 는 분 단위 offset 만 (`INT`, DST 없는 한국에 충분)
--   - 기본값 540 = +9시간 = KST
--   - PCAGT_POLICY 의 모든 스코프 row 에 적용 (COMPANY/TEAM/EMPLOYEE)
--   - 운영자 PATCH /policy 로 변경 가능
--
-- 컬럼: `TIME_ZONE_OFFSET_MINUTES INT NOT NULL DEFAULT 540`
--   - 범위: -720 ~ +840 (UTC-12 ~ UTC+14)
--   - 변환: `DATEADD(minute, TIME_ZONE_OFFSET_MINUTES, utc_value)` = 로컬 시각
--
-- 후속 변경 (다음 마이그):
--   0011: `v_PCAGT_*_KST` 뷰를 `v_PCAGT_*_LOCAL` 로 재명명 + 회사 offset 적용
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) PCAGT_POLICY.TIME_ZONE_OFFSET_MINUTES 추가 (멱등)
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (
    SELECT 1 FROM sys.columns
     WHERE object_id = OBJECT_ID(N'dbo.PCAGT_POLICY')
       AND name      = N'TIME_ZONE_OFFSET_MINUTES'
)
BEGIN
    ALTER TABLE dbo.PCAGT_POLICY
        ADD TIME_ZONE_OFFSET_MINUTES INT NOT NULL
            CONSTRAINT DF_PCAGT_POL_TZ_OFFSET DEFAULT(540);
END
GO

-- 기존 row 백필 — DEFAULT 가 INT NOT NULL 이라 자동 채워지지만 명시.
UPDATE dbo.PCAGT_POLICY
   SET TIME_ZONE_OFFSET_MINUTES = 540
 WHERE TIME_ZONE_OFFSET_MINUTES IS NULL;
GO

-- 검증
-- SELECT POLICY_SID, POLICY_SCOPE, CMPSID, TIME_ZONE_OFFSET_MINUTES
--   FROM dbo.PCAGT_POLICY WHERE IS_ACTIVE = 1
--  ORDER BY POLICY_SID;
