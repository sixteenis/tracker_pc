# data/local — 로컬 SQLite 저장소

## ⚠️ 추후 서버 마이그레이션 예정 — 그러나 그대로 옮기면 안 되는 것들

| 테이블 | 역할 | 서버 이전 가능? |
|---|---|---|
| `auth` | 마지막 로그인 사용자 식별 정보 (자동로그인 동의 플래그 포함) | ✅ 가능 — 단, 자동로그인 동의 플래그는 어딘가 남겨야 |
| `local_events` | 서버 전송 대기 이벤트 큐 (PENDING/SUCCESS/FAILED) | ❌ **오프라인 큐잉 핵심 — 옮기면 오프라인 동작 X** |
| `idle_segments` | 자리비움/잠금/PC종료 구간 | ❌ **오프라인 큐잉 핵심** — 네트워크 끊긴 동안 발생한 구간 보관 |
| `explanations` | 사용자 입력 소명 | ❌ **오프라인 큐잉** — 제출 실패 시 재시도 대상 |
| `settings` | 단순 KV 캐시 (device_id 등) | ✅ 가능 — read-only 정책 캐시 정도 |

## 옮길 때 결정해야 하는 것

오프라인 큐잉 테이블(`local_events`, `idle_segments`, `explanations`)을 서버로 옮기면:
- 사용자 PC가 네트워크 끊긴 동안엔 자리비움/이벤트가 **유실**된다
- 즉 PC 앱의 핵심 기능(미사용 시간 자동 보정)이 무력화된다

따라서 두 가지 길:
1. **오프라인 큐잉만 로컬 유지** — 위 표의 ❌ 만 남기고 나머지(`auth`, `settings`)는 서버 / OS 설정으로 이전
2. **모두 서버로** — 오프라인 가용성 포기 의식적 결정 + UX(네트워크 끊김 안내) 보강

## 파일 구성

- `mod.rs` — `Database` 연결 + 마이그레이션 진입점
- `auth_repo.rs` — `auth` 테이블
- `events_repo.rs` — `local_events` 테이블
- `idle_segments_repo.rs` — `idle_segments` 테이블
- `explanations_repo.rs` — `explanations` 테이블
- `settings_repo.rs` — `settings` 테이블

마이그레이션 SQL 은 `migrations/0001_init.sql` 에 그대로 남겨둔다.
