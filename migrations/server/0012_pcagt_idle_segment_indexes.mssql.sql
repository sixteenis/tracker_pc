-- ============================================================================
-- 핀플 PC Agent V2 — PCAGT_IDLE_SEGMENT 조회 성능 인덱스 보강 (2026-05-14)
-- ----------------------------------------------------------------------------
-- 배경:
--   `GET /api/pc-agent/worktime-explanations` 의 쿼리가 EMPSID 필터 + WORK_DATE
--   범위 + ORDER BY START_TIME DESC 패턴인데, 현재 인덱스 (`IX_PCAGT_SEG_EMP_DATE`
--   = EMPSID, WORK_DATE DESC) 가 ORDER BY 매칭 안 됨 → 매 호출마다 Sort 발생 +
--   Key Lookup. 한 사용자가 수만 건 누적 시 응답 지연.
--
-- 변경:
--   1) IX_PCAGT_SEG_EMP_START_LIST 신규 (covering index)
--      - 키: (EMPSID, START_TIME DESC)
--      - WHERE 필터 (EMPSID) + ORDER BY (START_TIME DESC) 한 인덱스로 처리
--      - INCLUDE: 응답에 자주 들어가는 컬럼들 — Key Lookup 회피
--      - WHERE EXPLANATION_REQUIRED = 1 filtered — 실제 소명 대상 row 만 인덱싱
--
-- 기존 인덱스는 유지 (다른 쿼리 패턴에서 활용 — 예: 통계 SQL).
--
-- 멱등성: IF NOT EXISTS 가드.
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1) IX_PCAGT_SEG_EMP_START_LIST — worktime-explanations 응답용 covering index
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (
    SELECT 1 FROM sys.indexes
     WHERE name = N'IX_PCAGT_SEG_EMP_START_LIST'
       AND object_id = OBJECT_ID(N'dbo.PCAGT_IDLE_SEGMENT')
)
BEGIN
    CREATE NONCLUSTERED INDEX IX_PCAGT_SEG_EMP_START_LIST
        ON dbo.PCAGT_IDLE_SEGMENT (EMPSID, START_TIME DESC)
        INCLUDE (
            WORK_DATE, END_TIME, DURATION_SECONDS,
            SEGMENT_TYPE, APPLIED_IDLE_THRESHOLD_SECONDS,
            EXPLANATION_DEADLINE, EXPLANATION_STATUS,
            SEGMENT_ID
        )
        WHERE EXPLANATION_REQUIRED = 1;
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 검증 쿼리 (적용 후)
-- ────────────────────────────────────────────────────────────────────────────
-- 1) 인덱스 추가 확인
-- SELECT name, filter_definition
--   FROM sys.indexes
--  WHERE object_id = OBJECT_ID(N'dbo.PCAGT_IDLE_SEGMENT');
--
-- 2) 실행 계획 확인 (SET STATISTICS IO ON 으로 페이지 읽기 비교)
-- SET STATISTICS IO ON;
-- SET STATISTICS TIME ON;
-- SELECT TOP 500 SEGMENT_ID, WORK_DATE, START_TIME, END_TIME, DURATION_SECONDS
--   FROM dbo.PCAGT_IDLE_SEGMENT
--  WHERE EMPSID = 48660
--    AND WORK_DATE >= CAST(DATEADD(DAY, -7, SYSDATETIME()) AS DATE)
--    AND EXPLANATION_REQUIRED = 1
--    AND (EXPLANATION_DEADLINE IS NULL OR EXPLANATION_DEADLINE >= SYSUTCDATETIME())
--  ORDER BY START_TIME DESC;
-- → 새 인덱스 (IX_PCAGT_SEG_EMP_START_LIST) Seek + ORDER BY 정렬 비용 0
