# WayNudge MVP 검증 기록

2026-10-25, Rust 1.98.1 / Cargo 1.98.1, Linux에서 실제 실행했습니다. 기본 제공자는 Mock이며 실제 ODsay API 키는 제공되지 않았습니다.

## 자동 검증

| 명령 | 결과 |
| --- | --- |
| `cargo fmt --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS, 경고 없음 |
| `cargo test --locked` | PASS, 22개, 실패·무시 0개 |
| `cargo build --locked` | PASS |
| `bash -n scripts/dev.sh scripts/tunnel.sh` | PASS |
| `actionlint .github/workflows/ci.yml` (v1.7.12) | PASS |
| `quick_validate.py` 세 Skill, 격리된 PyYAML 환경 | PASS 3개 |

테스트는 실제 SQLite와 공식 rmcp HTTP transport를 사용합니다. HTTP transport 테스트는 Axum Router를 직접 호출하며, 아래 수동 검증은 실제 TCP 서버와 외부 HTTPS를 호출했습니다.

| 요구 동작 | 테스트 |
| --- | --- |
| 1분 빠른 대안 유지 | `holds_for_one_minute_saving` |
| 큰 지연 시 변경 | `switches_for_large_fresh_delay`, `simulated_delay_changes_route_and_enqueues_disruption_alert` |
| 환승 실패 시 현재 위치에서 제외 경로 없이 재탐색 | `missed_transfer_replans_only_from_reported_origin` |
| API 오류에 Mock 실시간 결과를 만들지 않음 | `http_failure_is_private_and_consumes_quota` (실제 로컬 HTTP 503 fixture) |
| 휴가·설정 공휴일·자정 경계 제외 | `departure_and_alerts_follow_exclusions_and_deduplication`, `configured_holiday_suppresses_alert_until_explicit_override`, `overnight_alert_uses_arrival_date_exclusion` |
| 중복·최소 간격·동시 처리·재시작 영속성 | `alerts_deduplicate_and_space_across_midnight_and_kinds`, `concurrent_connections_emit_once_and_survive_reopening` |
| 사용자 격리·변경 확인 | `routines_are_isolated_when_ids_overlap`, `authentication_and_confirmation_protect_http_data` |
| MCP 7개 실제 도구 호출 | `seven_tools_are_callable_over_stateless_json_http` |
| 도보 보정·오래된 지연 무시 | `filters_incumbent_using_adjusted_walking_and_mode`, `warns_and_ignores_stale_delay` |

## 실제 HTTP 및 Cloudflare

[원본 응답: 헤더·상태·본문](evidence/live-http.md).

- `curl -i http://127.0.0.1:8080/health`: HTTP401 + Bearer 인증 요구.
- 같은 요청에 검증용 Bearer 토큰: HTTP200 `{"status":"ok"}`.
- 확인 전 루틴 저장: HTTP409, 확인 후 HTTP200 `saved=true`.
- 저장 사용자 목록에는 루틴 1개, 다른 사용자는 빈 목록.
- 09:00 도착, 지하철38분, 여유10분, 준비30분: 출발08:12·준비07:42(+09:00).
- 모의 지하철 지연25분: 혼합 경로로 전환, `should_switch=true`, `source=mock`, `realtime=false` 및 모의 장애 경고.
- 로컬 `/mcp` initialize·tools/list·tools/call: HTTP200 `application/json`, 7개 입출력 스키마.
- `scripts/tunnel.sh`로 실제 랜덤 주소 `https://pie-beautiful-refuse-levels.trycloudflare.com` 발급 및 QUIC 연결 성공.
- 외부 주소 `/health`: 익명 HTTP401, 인증 HTTP200.
- 외부 주소 `/mcp` tools/call: HTTP200 JSON + 저장 루틴 반환. SSE 연결 불필요.

이 주소는 검증 후 터널을 종료하여 현재 사용할 수 없습니다. 서버·터널 프로세스를 종료했고 검증 DB와 임시 도구는 실행 종료 시 정리합니다. 영구 배포 주소가 아닙니다.

## CI/CD

[개인 비공개 저장소](https://github.com/Helloworld0822/waynudge)의 [Actions 실행 결과](https://github.com/Helloworld0822/waynudge/actions/workflows/ci.yml)에서 커밋별 원본 로그를 확인할 수 있습니다. 검증 성공 후 실행 파일 artifact와 GHCR 컨테이너를 발행합니다. 로컬 Docker 데몬은 실행 중이 아니어서 로컬 컨테이너 실행은 검증하지 못했습니다. Rust1.98-bookworm 이미지 manifest는 조회에 성공했습니다.

## 제한과 후속 과제

- ODsay 실계정 호출·계정별 권한·그래픽 상표 표시 준수는 검증하지 못했습니다. 도시권 경로 예상치 어댑터와 HTTP 오류 fixture를 검증했습니다. ODsay Basic30회/일 및 저장·가공·배포 제약은 [공식 정책](https://lab.odsay.com/doc/totalPolicy)을 확인했습니다. 캐시는 기본0초입니다.
- 실제 버스·열차 지연 수집은 연결하지 않았습니다. 경로 변경 엔진과 추가 알림은 모의 지연으로 검증했습니다. 시외 접근·고속·항공·출발시각별 시간표 검증 및 승강장 이동 안내는 지원하지 않습니다.
- 알림은 SQLite 폴링 inbox입니다. 기기 푸시·메신저·메일 전송이 없으며 클라이언트가 수신함을 읽어야 합니다. 스케줄러는 도착6시간 전부터 작동하고 서버 중단 중 알림을 보충하지 않습니다.
- 한국 공휴일은 `HOLIDAYS`에 운영자가 설정해야 합니다. 제외일은 도착 날짜 기준이며 명시적 `enabled=true`가 우선합니다.
- API 한도·캐시는 프로세스별 메모리입니다. 재시작·다중 인스턴스의 전역 한도는 향후 SQLite 영속 카운터로 개선해야 합니다.
- 피드백은 실제 소요시간을 저장하며 도보 배율은 사용자가 준 경우만 갱신합니다. 학습·검증하지 않은 지연 확률은 생성하지 않습니다.
- SQLite 암호화·보존 기간·읽음 처리·삭제 API·OAuth는 제공하지 않습니다. 서버 파일 권한과 백업은 운영자가 관리합니다.
- LSP 데몬이 반복적으로 timeout되어 LSP 진단 완료를 주장하지 않습니다. Rust compiler·Clippy 진단과 동작 테스트는 통과했습니다.
