-- ============================================================================
-- 핀플 PC Agent V2 백엔드 전용 테이블 (MSSQL)
-- ----------------------------------------------------------------------------
-- 인증 방식:
--   로그인은 기존 V1 엔드포인트(/android/check_mbr.jsp) 의 이메일/비밀번호
--   검증을 그대로 사용한다. 토큰(access/refresh) 발급 / 갱신 / 만료 개념은
--   사용하지 않으며, V2 API 들은 매 호출마다 식별자(EMPSID/CMPSID/DEVICE_ID)
--   를 요청 본문 또는 쿼리로 받아 처리한다.
--
-- 대상 V2 API (docs/api.md):
--   3  GET  /api/pc-agent/policy                 (PCAGT_POLICY)
--   4  GET  /api/pc-agent/update-check           (PCAGT_APP_VERSION)
--   5  POST /api/pc-agent/heartbeat              (PCAGT_HEARTBEAT, PCAGT_DEVICE_SESSION)
--   6  POST /api/pc-agent/events                 (PCAGT_EVENT)
--   7  GET  /api/pc-agent/worktime-explanations  (PCAGT_IDLE_SEGMENT)
--   8  POST /api/pc-agent/worktime-explanations  (PCAGT_EXPLANATION)
--   9  GET  /api/pc-agent/attendance-status      (PCAGT_ATTENDANCE_SNAPSHOT)
--
-- 제외 — 기존 V1 테이블 / 인증 그대로 사용:
--   1  POST /api/pc-agent/login                  (check_mbr 의 이메일+비밀번호 인증)
--      ※ 로그인 성공 시 PCAGT_DEVICE_SESSION 에 디바이스 row 를 upsert 한다.
--   2  POST /api/pc-agent/refresh                — 사용하지 않음 (토큰 미사용).
--   ·  GET  /android/check_pay_use.jsp
--   ·  GET  /android/u/get_main2.jsp
--
-- 식별자 매핑 (모두 기존 PK 값을 그대로 저장하며 FK 제약은 걸지 않는다):
--   CMPSID  INT  — 회사   (회사 테이블 PK)
--   MBRSID  INT  — 회원   (회원 테이블 PK)
--   TTMSID  INT  — 상위팀 (상위팀 테이블 PK)
--   TEMSID  INT  — 팀     (팀 테이블 PK)
--   EMPSID  INT  — 근로자 (근로자 테이블 PK)
--
-- 시간 컬럼은 모두 UTC 저장 (DATETIME2). 기획서가 RFC3339 (UTC) 임을 따른다.
-- ============================================================================

SET NOCOUNT ON;
GO

-- ────────────────────────────────────────────────────────────────────────────
-- 1. PCAGT_DEVICE_SESSION
--    "어떤 근로자가 어느 PC 에 현재 로그인되어 있는가" 만 추적한다. 토큰은
--    저장하지 않는다 (인증은 매 호출마다 EMPSID/CMPSID + 기존 check_mbr 결과로
--    확인).
--
--    동일 EMPSID 가 다른 PC 에서 신규 로그인하면 기존 row 의 IS_ACTIVE = 0 +
--    DISPLACED_AT 채움 → 신규 row INSERT. 로그인 응답의 displaced_device 는
--    직전 활성 row 에서 만들어 내려준다.
--
--    같은 EMPSID 가 같은 DEVICE_ID 로 재로그인하면 같은 row 를 UPDATE
--    (LAST_LOGIN_AT, APP_VERSION 등 갱신, IS_ACTIVE 는 1 로 복귀).
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_DEVICE_SESSION') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_DEVICE_SESSION (
        SESSION_SID            BIGINT          IDENTITY(1,1) NOT NULL,

        CMPSID                 INT             NOT NULL,
        MBRSID                 INT             NOT NULL,
        EMPSID                 INT             NOT NULL,
        TTMSID                 INT             NULL,
        TEMSID                 INT             NULL,

        DEVICE_ID              NVARCHAR(64)    NOT NULL,   -- 클라이언트 발급 UUID
        DEVICE_NAME            NVARCHAR(200)   NOT NULL,
        OS                     NVARCHAR(20)    NOT NULL,   -- 'windows' / 'macos'
        APP_VERSION            NVARCHAR(40)    NOT NULL,

        IS_ACTIVE              BIT             NOT NULL CONSTRAINT DF_PCAGT_DS_ACT  DEFAULT(1),
        DISPLACED_AT           DATETIME2(0)    NULL,
        DISPLACED_REASON       NVARCHAR(40)    NULL,       -- 'OTHER_DEVICE_LOGIN' / 'FORCE_LOGOUT' 등

        LAST_LOGIN_AT          DATETIME2(0)    NOT NULL,
        LAST_HEARTBEAT_AT      DATETIME2(0)    NULL,

        REG_DT                 DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_DS_REG  DEFAULT(SYSUTCDATETIME()),
        UPD_DT                 DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_DS_UPD  DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_DEVICE_SESSION PRIMARY KEY CLUSTERED (SESSION_SID)
    );

    -- 한 근로자 + 같은 DEVICE_ID 조합은 항상 단일 row 로 유지 (재로그인 시 UPDATE).
    CREATE UNIQUE INDEX UX_PCAGT_DS_EMP_DEV    ON dbo.PCAGT_DEVICE_SESSION (EMPSID, DEVICE_ID);
    CREATE INDEX        IX_PCAGT_DS_EMP_ACTIVE ON dbo.PCAGT_DEVICE_SESSION (EMPSID, IS_ACTIVE);
    CREATE INDEX        IX_PCAGT_DS_HB_TIME    ON dbo.PCAGT_DEVICE_SESSION (LAST_HEARTBEAT_AT);
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 2. PCAGT_POLICY
--    회사 / 팀 / 근로자 스코프별 idle 정책. 우선순위는
--      EMPLOYEE > TEAM > COMPANY > (서버 기본값)
--    이며 백엔드가 조회 시 effective_* 값을 직접 계산해서 응답한다.
--    한 스코프당 행은 하나만 활성(IS_ACTIVE=1) 이며, 변경 시 신규 행을 추가하고
--    이전 행을 IS_ACTIVE=0 으로 내리는 append-only 운용을 권장한다.
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_POLICY') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_POLICY (
        POLICY_SID                     BIGINT          IDENTITY(1,1) NOT NULL,

        POLICY_SCOPE                   NVARCHAR(20)    NOT NULL,    -- 'COMPANY'/'TEAM'/'EMPLOYEE'
        CMPSID                         INT             NOT NULL,    -- 모든 스코프에 회사 키는 필수
        TTMSID                         INT             NULL,        -- TEAM 스코프(상위팀)
        TEMSID                         INT             NULL,        -- TEAM 스코프(팀)
        EMPSID                         INT             NULL,        -- EMPLOYEE 스코프

        IDLE_THRESHOLD_SECONDS         INT             NULL,        -- NULL = 상위 스코프 상속
        LUNCH_START_TIME               TIME(0)         NULL,
        LUNCH_END_TIME                 TIME(0)         NULL,
        LUNCH_ALLOWED_MINUTES          INT             NULL,
        EXPLANATION_DEADLINE_HOURS     INT             NULL,
        HEARTBEAT_INTERVAL_SECONDS     INT             NULL,
        EVENT_BATCH_INTERVAL_SECONDS   INT             NULL,
        CAN_TRACK_TIME                 BIT             NOT NULL CONSTRAINT DF_PCAGT_POL_CTT DEFAULT(1),

        POLICY_VERSION                 BIGINT          NOT NULL,    -- 회사 단위로 단조 증가
        IS_ACTIVE                      BIT             NOT NULL CONSTRAINT DF_PCAGT_POL_ACT DEFAULT(1),

        REG_DT                         DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_POL_REG DEFAULT(SYSUTCDATETIME()),
        UPD_DT                         DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_POL_UPD DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_POLICY PRIMARY KEY CLUSTERED (POLICY_SID),
        CONSTRAINT CK_PCAGT_POL_SCOPE CHECK (POLICY_SCOPE IN ('COMPANY','TEAM','EMPLOYEE')),
        CONSTRAINT CK_PCAGT_POL_SCOPE_KEYS CHECK (
            (POLICY_SCOPE = 'COMPANY'  AND EMPSID IS NULL AND TEMSID IS NULL AND TTMSID IS NULL)
         OR (POLICY_SCOPE = 'TEAM'     AND EMPSID IS NULL AND TEMSID IS NOT NULL)
         OR (POLICY_SCOPE = 'EMPLOYEE' AND EMPSID IS NOT NULL)
        )
    );

    CREATE INDEX IX_PCAGT_POL_CMP  ON dbo.PCAGT_POLICY (CMPSID, IS_ACTIVE);
    CREATE INDEX IX_PCAGT_POL_TEAM ON dbo.PCAGT_POLICY (TEMSID, IS_ACTIVE) WHERE TEMSID IS NOT NULL;
    CREATE INDEX IX_PCAGT_POL_EMP  ON dbo.PCAGT_POLICY (EMPSID, IS_ACTIVE) WHERE EMPSID IS NOT NULL;

    -- 활성 정책은 스코프당 1행만 존재하도록 필터 인덱스로 강제.
    CREATE UNIQUE INDEX UX_PCAGT_POL_CMP_ACT  ON dbo.PCAGT_POLICY (CMPSID)
        WHERE IS_ACTIVE = 1 AND POLICY_SCOPE = 'COMPANY';
    CREATE UNIQUE INDEX UX_PCAGT_POL_TEAM_ACT ON dbo.PCAGT_POLICY (CMPSID, TEMSID)
        WHERE IS_ACTIVE = 1 AND POLICY_SCOPE = 'TEAM';
    CREATE UNIQUE INDEX UX_PCAGT_POL_EMP_ACT  ON dbo.PCAGT_POLICY (CMPSID, EMPSID)
        WHERE IS_ACTIVE = 1 AND POLICY_SCOPE = 'EMPLOYEE';
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 3. PCAGT_APP_VERSION
--    OS별 PC Agent 최신/최소 요구 버전. 활성 행은 OS 당 1개.
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_APP_VERSION') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_APP_VERSION (
        APPVER_SID                 BIGINT          IDENTITY(1,1) NOT NULL,
        OS                         NVARCHAR(20)    NOT NULL,        -- 'windows' / 'macos'
        LATEST_VERSION             NVARCHAR(40)    NOT NULL,
        MINIMUM_REQUIRED_VERSION   NVARCHAR(40)    NOT NULL,
        FORCE_UPDATE               BIT             NOT NULL CONSTRAINT DF_PCAGT_AV_FU  DEFAULT(0),
        DOWNLOAD_URL               NVARCHAR(500)   NOT NULL,
        RELEASE_NOTE               NVARCHAR(MAX)   NULL,
        IS_ACTIVE                  BIT             NOT NULL CONSTRAINT DF_PCAGT_AV_ACT DEFAULT(1),
        REG_DT                     DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_AV_REG DEFAULT(SYSUTCDATETIME()),
        UPD_DT                     DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_AV_UPD DEFAULT(SYSUTCDATETIME()),
        CONSTRAINT PK_PCAGT_APP_VERSION PRIMARY KEY CLUSTERED (APPVER_SID),
        CONSTRAINT CK_PCAGT_AV_OS CHECK (OS IN ('windows','macos'))
    );
    CREATE UNIQUE INDEX UX_PCAGT_AV_OS_ACT ON dbo.PCAGT_APP_VERSION (OS) WHERE IS_ACTIVE = 1;
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 4. PCAGT_HEARTBEAT
--    3분 주기 상태 보고. 운영 중 양이 많아질 수 있으므로 EMPSID + REG_DT 인덱스
--    기반 파티셔닝/롤링 삭제 정책을 함께 운용한다 (예: 30일 보관).
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_HEARTBEAT') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_HEARTBEAT (
        HB_SID                            BIGINT          IDENTITY(1,1) NOT NULL,

        CMPSID                            INT             NOT NULL,
        EMPSID                            INT             NOT NULL,
        DEVICE_ID                         NVARCHAR(64)    NOT NULL,
        DEVICE_NAME                       NVARCHAR(200)   NULL,
        APP_VERSION                       NVARCHAR(40)    NULL,

        PC_STATUS                         NVARCHAR(20)    NOT NULL,   -- ACTIVE/IDLE/LOCKED/APP_CLOSING/OFFLINE
        LAST_ACTIVITY_AT                  DATETIME2(0)    NOT NULL,
        IDLE_SECONDS                      INT             NOT NULL,
        IS_LOCKED                         BIT             NOT NULL,

        ATTENDANCE_STATUS                 NVARCHAR(20)    NULL,
        CAN_TRACK_TIME                    BIT             NOT NULL,
        EFFECTIVE_IDLE_THRESHOLD_SECONDS  INT             NULL,

        REG_DT                            DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_HB_REG DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_HEARTBEAT PRIMARY KEY CLUSTERED (HB_SID),
        CONSTRAINT CK_PCAGT_HB_PCSTATUS CHECK (PC_STATUS IN
            ('ACTIVE','IDLE','LOCKED','APP_CLOSING','OFFLINE'))
    );
    CREATE INDEX IX_PCAGT_HB_EMP_TIME ON dbo.PCAGT_HEARTBEAT (EMPSID, REG_DT DESC);
    CREATE INDEX IX_PCAGT_HB_DEV_TIME ON dbo.PCAGT_HEARTBEAT (DEVICE_ID, REG_DT DESC);
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 5. PCAGT_EVENT
--    클라이언트 배치 이벤트 큐의 서버 측 저장소. EVENT_ID 가 클라이언트에서
--    생성된 UUID 라 UNIQUE 제약으로 멱등 처리.
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_EVENT') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_EVENT (
        EVENT_SID         BIGINT          IDENTITY(1,1) NOT NULL,
        EVENT_ID          CHAR(36)        NOT NULL,           -- 클라이언트 UUID
        CMPSID            INT             NOT NULL,
        EMPSID            INT             NOT NULL,
        DEVICE_ID         NVARCHAR(64)    NOT NULL,

        EVENT_TYPE        NVARCHAR(40)    NOT NULL,
        EVENT_TIME        DATETIME2(0)    NOT NULL,
        PAYLOAD_JSON      NVARCHAR(MAX)   NULL,               -- ISJSON 검증

        RECEIVED_AT       DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_EVT_RECV DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_EVENT PRIMARY KEY CLUSTERED (EVENT_SID),
        CONSTRAINT UX_PCAGT_EVENT_ID UNIQUE (EVENT_ID),
        CONSTRAINT CK_PCAGT_EVT_PAYLOAD CHECK (PAYLOAD_JSON IS NULL OR ISJSON(PAYLOAD_JSON) = 1),
        CONSTRAINT CK_PCAGT_EVT_TYPE CHECK (EVENT_TYPE IN (
            'APP_STARTED','APP_STOPPED','AUTO_LOGIN_SUCCESS','AUTO_LOGIN_FAILED',
            'USER_ACTIVE','IDLE_STARTED','IDLE_ENDED','PC_LOCKED','PC_UNLOCKED',
            'PC_SHUTDOWN_DETECTED','NO_PC_RECORD','HEARTBEAT','EXPLANATION_SUBMITTED',
            'SYNC_SUCCESS','SYNC_FAILED'
        ))
    );
    CREATE INDEX IX_PCAGT_EVT_EMP_TIME  ON dbo.PCAGT_EVENT (EMPSID, EVENT_TIME DESC);
    CREATE INDEX IX_PCAGT_EVT_TYPE_TIME ON dbo.PCAGT_EVENT (EVENT_TYPE, EVENT_TIME DESC);
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 6. PCAGT_IDLE_SEGMENT
--    자리비움/잠금/앱종료/PC종료/기록없음 구간. GET worktime-explanations 응답 소스.
--    SEGMENT_ID 는 클라이언트가 IDLE_STARTED 이벤트와 함께 발급한 UUID 그대로 보관.
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_IDLE_SEGMENT') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_IDLE_SEGMENT (
        SEG_SID                           BIGINT          IDENTITY(1,1) NOT NULL,
        SEGMENT_ID                        CHAR(36)        NOT NULL,         -- 클라이언트 UUID

        CMPSID                            INT             NOT NULL,
        EMPSID                            INT             NOT NULL,
        DEVICE_ID                         NVARCHAR(64)    NOT NULL,

        WORK_DATE                         DATE            NOT NULL,
        SEGMENT_TYPE                      NVARCHAR(20)    NOT NULL,
        START_TIME                        DATETIME2(0)    NOT NULL,
        END_TIME                          DATETIME2(0)    NULL,
        DURATION_SECONDS                  INT             NULL,

        APPLIED_IDLE_THRESHOLD_SECONDS    INT             NOT NULL,
        POLICY_SCOPE                      NVARCHAR(20)    NOT NULL,

        EXPLANATION_REQUIRED              BIT             NOT NULL CONSTRAINT DF_PCAGT_SEG_REQ  DEFAULT(1),
        EXPLANATION_DEADLINE              DATETIME2(0)    NULL,
        EXPLANATION_STATUS                NVARCHAR(20)    NOT NULL CONSTRAINT DF_PCAGT_SEG_STAT DEFAULT('PENDING'),
        WORKTIME_REFLECTION_STATUS        NVARCHAR(40)    NULL,             -- BREAK_CANDIDATE/WORK_CONFIRMED 등

        REG_DT                            DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_SEG_REG  DEFAULT(SYSUTCDATETIME()),
        UPD_DT                            DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_SEG_UPD  DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_IDLE_SEGMENT PRIMARY KEY CLUSTERED (SEG_SID),
        CONSTRAINT UX_PCAGT_SEG_ID UNIQUE (SEGMENT_ID),
        CONSTRAINT CK_PCAGT_SEG_TYPE CHECK (SEGMENT_TYPE IN
            ('PC_IDLE','PC_LOCKED','PC_APP_CLOSED','PC_SHUTDOWN','NO_PC_RECORD')),
        CONSTRAINT CK_PCAGT_SEG_SCOPE CHECK (POLICY_SCOPE IN
            ('COMPANY','TEAM','EMPLOYEE','DEFAULT')),
        CONSTRAINT CK_PCAGT_SEG_STATUS CHECK (EXPLANATION_STATUS IN
            ('PENDING','SUBMITTED','EXPIRED','EXEMPTED'))
    );
    CREATE INDEX IX_PCAGT_SEG_EMP_DATE ON dbo.PCAGT_IDLE_SEGMENT (EMPSID, WORK_DATE DESC);
    CREATE INDEX IX_PCAGT_SEG_STATUS   ON dbo.PCAGT_IDLE_SEGMENT (EXPLANATION_STATUS, EXPLANATION_DEADLINE);
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 7. PCAGT_EXPLANATION
--    POST worktime-explanations 로 들어온 소명 제출 내역.
--    한 segment 에 대해 재제출 이력을 남기려면 단순 append 후 최신 row 만 노출.
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_EXPLANATION') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_EXPLANATION (
        EXP_SID             BIGINT          IDENTITY(1,1) NOT NULL,
        SEGMENT_ID          CHAR(36)        NOT NULL,
        CMPSID              INT             NOT NULL,
        EMPSID              INT             NOT NULL,

        EXPLANATION_TYPE    NVARCHAR(30)    NOT NULL,
        EXPLANATION_TEXT    NVARCHAR(1000)  NULL,
        SUBMITTED_FROM      NVARCHAR(20)    NOT NULL CONSTRAINT DF_PCAGT_EXP_FROM DEFAULT('PC_APP'),
        SUBMITTED_AT        DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_EXP_SUB  DEFAULT(SYSUTCDATETIME()),
        REG_DT              DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_EXP_REG  DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_EXPLANATION PRIMARY KEY CLUSTERED (EXP_SID),
        CONSTRAINT FK_PCAGT_EXP_SEG FOREIGN KEY (SEGMENT_ID)
            REFERENCES dbo.PCAGT_IDLE_SEGMENT (SEGMENT_ID),
        CONSTRAINT CK_PCAGT_EXP_TYPE CHECK (EXPLANATION_TYPE IN (
            'MEETING','PHONE_CALL','CUSTOMER_RESPONSE','BUSINESS_TRIP','OUTSIDE_WORK',
            'EDUCATION','WORK_WAITING','PC_ERROR','APP_ERROR','OTHER_WORK','LUNCH_BREAK','PERSONAL'
        )),
        CONSTRAINT CK_PCAGT_EXP_FROM CHECK (SUBMITTED_FROM IN ('PC_APP','MOBILE_APP','WEB'))
    );
    CREATE INDEX IX_PCAGT_EXP_SEG      ON dbo.PCAGT_EXPLANATION (SEGMENT_ID);
    CREATE INDEX IX_PCAGT_EXP_EMP_DATE ON dbo.PCAGT_EXPLANATION (EMPSID, SUBMITTED_AT DESC);
END
GO


-- ────────────────────────────────────────────────────────────────────────────
-- 8. PCAGT_ATTENDANCE_SNAPSHOT
--    GET /attendance-status 응답용 스냅샷. 모바일 출퇴근 앱 / 자동 출퇴근 로직이
--    upsert 하면 PC Agent 백엔드가 read-only 로 조회한다.
--    근로자 + 날짜 조합으로 단일 row 를 유지 (UPSERT).
-- ────────────────────────────────────────────────────────────────────────────
IF NOT EXISTS (SELECT 1 FROM sys.objects WHERE object_id = OBJECT_ID(N'dbo.PCAGT_ATTENDANCE_SNAPSHOT') AND type = N'U')
BEGIN
    CREATE TABLE dbo.PCAGT_ATTENDANCE_SNAPSHOT (
        ATT_SID            BIGINT          IDENTITY(1,1) NOT NULL,
        CMPSID             INT             NOT NULL,
        EMPSID             INT             NOT NULL,
        WORK_DATE          DATE            NOT NULL,
        ATTENDANCE_STATUS  NVARCHAR(20)    NOT NULL,
        WORK_START_AT      DATETIME2(0)    NULL,
        WORK_END_AT        DATETIME2(0)    NULL,
        SOURCE             NVARCHAR(20)    NULL,                    -- 'MOBILE_APP'/'PC_APP'/'AUTO'

        REG_DT             DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_ATT_REG DEFAULT(SYSUTCDATETIME()),
        UPD_DT             DATETIME2(0)    NOT NULL CONSTRAINT DF_PCAGT_ATT_UPD DEFAULT(SYSUTCDATETIME()),

        CONSTRAINT PK_PCAGT_ATTENDANCE_SNAPSHOT PRIMARY KEY CLUSTERED (ATT_SID),
        CONSTRAINT UX_PCAGT_ATT_EMP_DATE UNIQUE (EMPSID, WORK_DATE),
        CONSTRAINT CK_PCAGT_ATT_STATUS CHECK (ATTENDANCE_STATUS IN
            ('WORKING','BEFORE_WORK','AFTER_WORK','OUTING','LEAVE','BUSINESS_TRIP','UNKNOWN'))
    );
    CREATE INDEX IX_PCAGT_ATT_CMP_DATE ON dbo.PCAGT_ATTENDANCE_SNAPSHOT (CMPSID, WORK_DATE);
END
GO
