# 실시간 연동 및 배포 검증

2026-10-25, `https://waynudge.helloworld0822.xyz/mcp`에서 실제 외부 HTTPS 요청으로 확인했습니다. 이 주소는 인증을 마친 Cloudflare named tunnel을 사용합니다. 랜덤 무료 주소를 사용하는 Quick Tunnel 실행 스크립트도 유지합니다. 서버가 실행되는 호스트가 켜져 있어야 합니다.

## 관측 결과

최종 소스에서 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, `cargo build --locked`가 통과했습니다. 테스트는 총 32개이며 실패·무시 0개입니다. 혼잡도 변환, 인증 격리, 실제 rmcp HTTP 도구 호출, 버스 접근 구간과 도보 속도 보정의 분리를 포함합니다.

| 요청 | 결과 |
| --- | --- |
| 익명 MCP initialize | HTTP 200, JSON |
| 익명 tools/list | HTTP 200, 입출력 스키마를 갖춘 10개 도구 |
| 서울 지하철: 서울, 1001, up | `source=seoul_subway`, 도착정보 5건 |
| TAGO: DJB8001793, 도시 25 | `source=tago`, 도착정보 4건 |
| 경기도 버스: 200000078 | `source=gyeonggi_bus`, 도착정보 22건 |
| 익명 list_routines | 빈 목록, 개인 루틴 미노출 |

응답 건수는 위 시점의 관측이며 시간에 따라 달라집니다. 오래된 지하철 관측값에는 경고가 붙습니다. 경기도 샘플에서는 잔여좌석을 확인했으며 혼잡도 값은 내려오지 않았습니다. 혼잡도 코드를 포함한 응답 파싱은 자동 테스트로 확인했습니다.

[경기버스정보 공식 도착정보 명세](https://www.gbis.go.kr/gbis2014/publicService.action?cmd=mBusArrivalStation)에 따라 `crowded1/2`를 여유·보통·혼잡·매우혼잡으로 변환하고 `remainSeatCnt1/2`의 음수는 정보 없음으로 처리합니다. 제공 여부는 노선 유형에 따라 다릅니다. SK API는 사용하지 않습니다. 현재 연결한 서울 지하철 도착정보와 TAGO 도착정보에서 열차 혼잡도를 추정하지 않습니다.

## 인증과 운영

공개 MCP로 실시간 교통정보를 조회할 수 있습니다. 개인 루틴과 변경 작업은 Bearer 인증이 필요하며 변경 확인도 유지합니다. HTTP API는 인증이 필요합니다. 익명 쓰기 거부, 개인 데이터 격리, 잘못된 Bearer 거부를 자동 테스트로 확인했습니다.

`waynudge.service`와 `waynudge-tunnel.service`는 사용자 systemd 서비스로 활성화되어 있습니다. 사용자 linger를 켰으며 데이터베이스는 사용자 데이터 디렉터리에 저장합니다. 키는 Git에서 제외된 `.env`, 서버 인증 토큰은 별도 권한 600 파일에 저장합니다.

서울 지하철 공급자의 공식 HTTP 엔드포인트를 사용합니다. HTTPS 연결은 이 환경에서 타임아웃이 발생했습니다. 외부 MCP 접속은 HTTPS지만 서울 공급자로 향하는 구간에는 TLS가 없습니다. 오류에 키나 요청 URL을 기록하지 않습니다.

## 알림과 경로의 범위

Telegram과 서버의 주기 알림 전송은 사용하지 않습니다. `schedule-commute` Skill과 `prepare_commute_reminder`가 호스트 모델의 예약 도구를 위한 일정과 프롬프트를 만듭니다. 공휴일·대체공휴일 제외는 해당 프롬프트에 포함됩니다. 호스트 예약 성공 전에는 `created=false`이며 이번 배포에서 실제 반복 알림은 등록하지 않았습니다.

승차 안내는 준비·출발·계획 승차 시각과 관측된 열차 도착을 구분합니다. 도보 속도 보정은 승차 전 도보 구간에만 적용합니다. 실시간 도착정보를 전체 경로의 실측 지연으로 바꾸지 않습니다. ODsay 키가 없으면 경로 비교는 명시적인 Mock 데이터이며, 별도 실시간 도착 조회는 실제 공급자 데이터입니다.
