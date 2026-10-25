# WayNudge

Rust로 실행하는 개인 출퇴근 의사결정 서버입니다. 저장한 루틴과 교통 경로를 바탕으로 준비·출발 시각, 경로 유지·변경 여부를 HTTP와 MCP로 제공합니다. 기본값은 **모의 데이터**이며 실제 운행 안내가 아닙니다.

## 구성

| 위치 | 역할 |
| --- | --- |
| `src/main.rs`, `src/api.rs`, `src/api/auth.rs` | 로컬 서버, HTTP 경계, Bearer 인증 |
| `src/mcp.rs` | 공식 Rust SDK `rmcp` 3.7 기반 MCP 도구 |
| `src/service.rs`, `src/routing.rs` | 서울 기준 시각과 경로 비교 |
| `src/routines.rs`, `src/storage.rs` | 루틴 검증과 사용자별 SQLite 저장 |
| `src/providers.rs`, `src/providers/odsay.rs` | 모의 경로와 ODsay 연동 |
| `src/scheduler.rs`, `src/alerts.rs` | 주기 평가와 중복 억제 수신함 |
| `.agents/skills/` | 한국어 출퇴근·재탐색·루틴 관리 스킬 |
| `examples/routine.json` | 확인 전 상태의 루틴 등록 예시 |
| `docs/verification.md` | 실제 실행한 검증 결과와 제한 사항 |

LLM은 결과 설명과 확인 절차를 담당하고, 시간 계산·경로 순위·전환 판단은 서버가 수행합니다. 주소 검색·지오코딩은 없으므로 출발지와 목적지 좌표를 함께 입력합니다.

## 로컬 실행

Linux/Bash, Rust 1.88 이상과 Cargo가 필요합니다. 예제 HTTP 명령은 `curl`, `jq`, 토큰 생성은 `openssl`을 사용합니다.

```bash
umask 077
cp .env.example .env
chmod 600 .env
openssl rand -hex 32
```

생성한 토큰을 `.env`의 `WAYNUDGE_USERS='{"me":"생성한 토큰"}'`에 넣습니다. 사용자별로 서로 다른 공백 없는 ASCII 토큰 32~256자가 필요합니다. 예시 토큰이나 실제 키를 저장소에 커밋하지 마세요.

```bash
bash scripts/dev.sh
```

서버는 `127.0.0.1:8080`에 바인딩합니다. 스크립트는 저장소 루트로 이동하고 `umask 077`을 적용한 뒤 `cargo run --locked`를 실행합니다. `.env`가 있으면 현재 사용자가 소유하고 다른 사용자 권한이 없는 일반 파일만 읽습니다. `.env`는 Bash 코드로 실행되므로 직접 작성한 신뢰할 수 있는 파일만 사용하세요. `.env` 없이 환경변수만 설정해도 실행할 수 있습니다.

| 환경변수 | 기본값 및 의미 |
| --- | --- |
| `WAYNUDGE_USERS` | 필수. 사용자 ID → 고유 토큰 JSON 객체 |
| `DATABASE_URL` | `sqlite://waynudge.db`. 루틴·피드백·날짜 예외·알림 저장 |
| `BIND_ADDR` | `127.0.0.1:8080`. 컨테이너는 `0.0.0.0:8080`, 인증은 항상 필수 |
| `TRANSIT_PROVIDER` | `mock`. 실제 요청은 `odsay` |
| `MOCK_SUBWAY_DELAY_MINUTES` | `0`. 0~180분의 모의 지하철 장애. 실시간 정보로 표시하지 않음 |
| `ODSAY_API_KEY` | `odsay`일 때 필수. 서버 전용 키 |
| `ODSAY_DAILY_LIMIT` | `30`. 1~30, 모든 사용자가 프로세스 한도를 공유 |
| `ODSAY_CACHE_SECONDS` | `0`. 캐시 끔. 명시적 저장 허용을 받은 경우만 1~300초 |
| `PUBLIC_HOST` | 미설정. 허용할 HTTPS 브라우저 Origin의 호스트명, 스킴·경로 제외 |
| `PUBLIC_MCP` | `false`. `true`이면 공개 MCP 교통 조회 허용. 개인 루틴에는 Bearer 인증 필요 |
| `SEOUL_SUBWAY_API_KEY` | 서울시 실시간 지하철 인증키 |
| `SEOUL_SUBWAY_DAILY_LIMIT` | `1000`. 프로세스 내부 호출 상한 |
| `SUBWAY_CONGESTION_SERVICE_KEY` | 서울교통공사 지하철혼잡도정보 Open API 인증키 |
| `SUBWAY_CONGESTION_DAILY_LIMIT` | `100`. 평균 혼잡도 조회의 프로세스 내부 호출 상한 |
| `TAGO_SERVICE_KEY` | 공공데이터포털 TAGO 인증키. 복사된 공백 및 URL 인코딩 키 정규화 |
| `GYEONGGI_SERVICE_KEY` | 경기도 버스 별도 키. 미설정·빈 값이면 TAGO 키 공유, 서비스별 활용 승인 필요 |
| `BUS_DAILY_LIMIT` | `1000`. 버스 제공자별 프로세스 내부 호출 상한 |

공휴일·대체공휴일 제외는 호스트 예약 알림 프롬프트에서 처리합니다. 서버에는 공휴일 API나 달력을 두지 않습니다. 특정 날짜의 `enabled` 설정은 루틴 요일보다 우선하며 휴가·재택 예외는 계속 지원합니다. 모든 루틴 시각은 `Asia/Seoul`입니다.

## HTTP 사용

HTTP API와 `/health`는 Bearer 인증이 필요합니다. `PUBLIC_MCP=true`이면 `/mcp`의 공개 교통 조회는 인증 없이 사용할 수 있습니다. 익명 호출에는 개인 루틴이 노출되지 않으며 루틴 저장·수정은 인증이 필요합니다. 사용자 ID는 요청 본문에서 받지 않고 토큰으로 결정합니다. 아래 명령은 저장소 루트의 별도 터미널에서 실행합니다.

```bash
set -a
source ./.env
set +a
export TOKEN="$(printf '%s' "$WAYNUDGE_USERS" | jq -r '.me')"
export BASE='http://127.0.0.1:8080'
export TRIP_DATE="$(TZ=Asia/Seoul date +%F)"

curl --fail-with-body -H "Authorization: Bearer $TOKEN" "$BASE/health"
curl --fail-with-body -H "Authorization: Bearer $TOKEN" "$BASE/api/routines"
```

`examples/routine.json`의 출발지·목적지·좌표·요일·09:00 도착·준비 30분·여유 10분·도보와 환승 제한을 본인 상황에 맞게 수정하세요. `confirmed=false`인 원본은 저장되지 않습니다. 내용을 확인하고 저장에 동의한 경우에만 다음 명령으로 `confirmed=true`를 전달합니다. 같은 ID는 전체 루틴을 갱신합니다.

```bash
jq '.confirmed = true' examples/routine.json |
  curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' --data-binary @- "$BASE/api/routines"

jq -n --arg day "$TRIP_DATE" \
  '{routine_id:"morning-commute",date:$day,current_route_id:null}' |
  curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' --data-binary @- "$BASE/api/brief"

jq -n --arg day "$TRIP_DATE" \
  '{routine_id:"morning-commute",date:$day,current_route_id:null}' |
  curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' --data-binary @- "$BASE/api/compare"
```

재탐색은 사용자가 제공한 현재 위치를 사용합니다. 아래 좌표는 시연용입니다. `stage`는 `at_station`, `walking`, `transfer_missed`이며, 환승 실패에는 확인된 `blocked_route_ids`가 하나 이상 필요합니다. 실제 경로 ID는 비교 응답에서 가져옵니다.

```bash
jq -n --arg day "$TRIP_DATE" \
  '{routine_id:"morning-commute",date:$day,current_route_id:null,
    current_location:{name:"서울역 예시",lat:37.5547,lon:126.9707},
    stage:"walking",blocked_route_ids:[]}' |
  curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' --data-binary @- "$BASE/api/replan"
```

날짜 예외는 아래 날짜의 출근을 제외하는 변경입니다. 사용자가 그 날짜의 제외를 명시적으로 확인한 뒤 실행하세요. `enabled=true`는 제외일에도 출근하도록 덮어씁니다.

```bash
jq -n --arg day "$TRIP_DATE" \
  '{routine_id:"morning-commute",date:$day,enabled:false,confirmed:true}' |
  curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
    -H 'Content-Type: application/json' --data-binary @- "$BASE/api/day-status"
```

피드백은 실제 소요 시간과 사용자가 확인한 경로 ID로 바꾸고, 저장할 내용을 확인한 뒤 실행합니다. 아래는 모의 경로용 예시입니다. `walking_factor`가 `null`이면 배율을 유지하며, 값을 지정하면 루틴의 배율도 갱신합니다. 같은 피드백을 재전송하면 중복 기록될 수 있습니다.

```bash
curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  --data '{"routine_id":"morning-commute","route_id":"mock-subway","actual_minutes":42,"walking_factor":null,"confirmed":true}' \
  "$BASE/api/feedback"

curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
  -X POST "$BASE/api/alerts/evaluate"
curl --fail-with-body -H "Authorization: Bearer $TOKEN" "$BASE/api/alerts"
```

`POST /api/alerts/evaluate`는 현재 시각에 해당하는 준비·경로 변경 알림을 평가해 수신함에 적재합니다. 백그라운드 알림 스케줄러는 실행하지 않으며 반복 알림은 호스트 모델 에이전트의 예약 기능을 사용합니다. `GET /api/alerts`를 클라이언트가 폴링해야 하며 모바일·메신저 푸시는 없습니다. 날짜·루틴·종류별 중복과 최소 알림 간격을 적용합니다. 휴일·제외일 또는 아직 준비 시각이 아닌 경우 비어 있는 수신함이 정상입니다.

수동 inbox 평가 API는 호환성을 위해 남아 있지만 텔레그램·별도 서버 푸시 전송기는 없습니다. `schedule-commute` Skill이 알림 시각과 요일을 받아 호스트의 실제 예약 도구로 등록하며, 매 실행 때 최신 브리핑을 조회합니다. 호스트에 예약 도구가 없으면 일정 초안만 반환하고 등록 성공으로 표시하지 않습니다.

## MCP 연결과 스킬

엔드포인트는 `http://127.0.0.1:8080/mcp`입니다. Streamable HTTP에서 **상태 없는 JSON 응답**을 사용하며 SSE 연결을 요구하지 않습니다. 클라이언트에 `Authorization: Bearer <토큰>` 헤더를 설정하세요. 이 서버는 OAuth 인증 흐름을 제공하지 않습니다.

```bash
curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"waynudge-example","version":"1.0"}}}' \
  "$BASE/mcp"

curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  --data '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' "$BASE/mcp"

curl --fail-with-body -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' \
  --data '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_routines","arguments":{}}}' \
  "$BASE/mcp"
```

| MCP 도구 | HTTP 대응 |
| --- | --- |
| `list_routines` | `GET /api/routines` |
| `upsert_routine` | `POST /api/routines` |
| `get_commute_brief` | `POST /api/brief` |
| `compare_routes` | `POST /api/compare` |
| `replan_trip` | `POST /api/replan` |
| `set_routine_day_status` | `POST /api/day-status` |
| `record_trip_feedback` | `POST /api/feedback` |
| `get_subway_arrivals` | `POST /api/subway/arrivals` |
| `get_subway_congestion` | `POST /api/subway/congestion` |
| `simulate_congestion` | `POST /api/simulate/congestion` |
| `get_bus_arrivals` | `POST /api/bus/arrivals` |
| `prepare_commute_reminder` | 호스트 예약용 일정·프롬프트 생성, 예약 자체를 생성하지 않음 |

도구 입력 스키마는 `tools/list`에서 확인합니다. 저장 도구에는 `confirmed`가 필수입니다. `true`는 사용자 확인을 표현하는 입력값일 뿐, 서버가 실제 대화를 검증하는 것은 아닙니다. 클라이언트는 변경 내용을 요약하고 명시적 확인을 받은 뒤 호출해야 합니다.

이 저장소의 `.agents/skills`를 읽는 에이전트에서 다음 스킬을 사용합니다.

- `plan-commute`: “오늘 언제 준비하고 출발해야 해?”
- `replan-trip`: “이 역에 도착했는데 환승을 놓쳤어. 여기서 다시 찾아줘.”
- `manage-routine`: “금요일은 재택으로 바꿔줘.”

스킬은 먼저 루틴을 조회하고, 서버 결과를 한국어로 설명합니다. 위치를 추적하거나 출발 시각을 자체 계산하지 않습니다.

## 무료 임시 HTTPS 터널

`cloudflared`를 [공식 Quick Tunnel 안내](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/)에 따라 미리 설치한 뒤 서버와 별도 터미널에서 실행합니다. 스크립트는 프로그램을 설치하거나 유료 도메인을 만들지 않습니다.

```bash
bash scripts/tunnel.sh
```

출력된 `https://임의이름.trycloudflare.com/mcp`를 MCP 주소로 사용합니다. 스크립트는 `--http-host-header localhost`를 사용해 로컬 MCP Host 허용 검사를 유지합니다. 인증 헤더는 여전히 필요합니다. 브라우저가 터널 주소의 Origin을 보내면 `.env`에 `PUBLIC_HOST='임의이름.trycloudflare.com'`을 추가한 후 서버를 재시작하세요. 이는 Origin 검증이며 범용 CORS 프록시는 아닙니다.

Quick Tunnel은 다시 만들 때 주소가 바뀌고 가동 시간 보장이 없으며 SSE를 지원하지 않습니다. 터널을 중지하면 주소도 동작하지 않습니다. 이 서버의 JSON 방식과 호환되는 클라이언트를 사용하고, 새 주소로 연결 설정과 필요시 `PUBLIC_HOST`를 갱신하세요. [Cloudflare 제한 사항](https://developers.cloudflare.com/tunnel/get-started/quick-tunnels/#limitations)

## ODsay 실제 데이터

ODsay 애플리케이션과 서버 API 키를 준비한 뒤 `TRANSIT_PROVIDER=odsay`와 `ODSAY_API_KEY`를 설정합니다. 키를 클라이언트나 MCP 인수에 넣지 않습니다. ODsay는 경로 예상치를 제공하고, 서울시 지하철·TAGO·경기도 버스 도착은 별도 어댑터로 조회합니다. 도착예정시간을 실제 전체 경로 지연으로 간주하거나 확률을 만들어내지 않습니다.

ODsay Basic은 개인·학생·5인 이하 스타트업 대상으로 하루 30회 무료 호출을 제공합니다. 본인의 이용 자격·등록 플랫폼·현재 한도를 확인하세요. 결과 데이터의 저장·가공·배포에는 사전 동의 조건이 있으므로 캐시는 기본적으로 꺼져 있습니다. 운영자는 실제 이용과 결과 전달 방식에 필요한 권한을 확인하고, 명시적 저장 허용을 받은 경우에만 캐시를 켜야 합니다. [ODsay 이용 정책](https://lab.odsay.com/doc/totalPolicy)

ODsay 결과를 표시하는 서비스는 소개·결과 화면 등의 상표 및 권리 귀속 표시 요구를 확인해야 합니다. 이 텍스트 API 예제만으로 로고 배치·크기 등 상표 준수를 완료했다고 주장하지 않습니다. 실제 클라이언트 공개 전 [ODsay 상표사용가이드](https://lab.odsay.com/guide/guide?platform=web#guide1_6)를 적용하세요.

## 판단과 한계

동적 변경을 끝까지 시연하려면 `TRANSIT_PROVIDER=mock`, `MOCK_SUBWAY_DELAY_MINUTES=25`로 재시작하고 `current_route_id:"mock-subway"`로 브리핑을 요청하세요. 경로 전환과 모의 장애 경고가 함께 반환됩니다. 평소 경로 ID도 `mock-subway`로 저장하면 스케줄러가 같은 조건에서 추가 경로 변경 알림을 수신함에 적재합니다. 실제 운행 장애를 수집하는 기능은 아닙니다.

- 준비 시각은 목표 도착에서 경로 소요 시간·여유·준비 시간을 빼서 산출합니다. 도보 배율은 도보 구간에 적용됩니다. 선호 수단·최대 환승·도보 제한은 엄격한 필터입니다.
- 현재 경로보다 최소 5분과 전환 부담을 넘어 이득이 있거나, 전환 부담을 반영해 목표 도착을 의미 있게 개선할 때 경로 전환을 권합니다. 오래된 실시간 지연값은 사용하지 않습니다.
- `mock`은 합성 경로입니다. `odsay`도 현재 실시간 지연을 가져오지 않으므로 도착 보장이 아닙니다. 결과의 `source`, `realtime`, `collected_at`, `warnings`를 함께 표시하세요.
- 후보가 없거나 날짜가 제외되면 출발·준비 시각은 `null`입니다. API 오류를 정상 경로로 바꾸지 않습니다. ODsay의 시외·고속·철도·항공 경로는 지원하지 않습니다.
- 자동 평가·브리핑·재탐색도 ODsay 호출을 사용합니다. 한도 카운터와 선택적 캐시는 프로세스 메모리에 있으며 재시작 시 초기화됩니다. 카운터 날짜는 UTC입니다. 다중 프로세스·재시작·외부 앱의 호출까지 합친 제공자 한도를 보장하지 않으므로 운영자가 실제 사용량을 관리해야 합니다.
- SQLite에는 출발·목적지 좌표와 루틴·피드백·알림이 저장됩니다. 저장 파일은 암호화되지 않습니다. 서버 파일 접근 권한과 백업을 관리하세요. 상시 위치 기록은 하지 않습니다.
- 날짜 예외 삭제·루틴 삭제·알림 읽음 처리·자동 보존 기간 정리 API는 없습니다. 실제 소요 시간 피드백은 기록되고, 도보 배율은 사용자가 제공한 경우에만 갱신됩니다.

## 검증

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
bash -n scripts/dev.sh scripts/tunnel.sh
```

실제로 실행한 시나리오, 결과, 증거 경로는 [검증 기록](docs/verification.md)을 참조하세요. 명령을 나열한 것만으로 실서비스·실제 ODsay 키·외부 터널이 검증됐다는 뜻은 아닙니다.

## GitHub Actions CI/CD

`develop`·`main` 브랜치 푸시, `v*` 태그, PR에서 fmt·Clippy·테스트·셸 문법 검사와 release 빌드를 실행합니다. 실행 파일은 `waynudge-linux-amd64` artifact로 14일 보관합니다. 성공한 푸시와 수동 실행은 `ghcr.io/helloworld0822/waynudge:latest` 및 커밋 SHA 태그로 컨테이너를 발행합니다. PR은 이미지를 발행하지 않습니다. Actions는 커밋 SHA에 고정했고 배포 작업에만 `packages:write` 권한을 부여합니다.

이는 실행 파일·이미지의 지속적 전달입니다. Quick Tunnel은 로컬 서버와 함께 실행해야 하며 Actions가 영구 서버나 고정 URL을 제공하지 않습니다. 실제 교통 키와 사용자 토큰은 이미지에 포함하지 않습니다.

Docker 데몬이 실행 중일 때 컨테이너를 직접 빌드할 수 있습니다.

```bash
docker build -t waynudge .
docker volume create waynudge-data
docker run --rm --name waynudge -p 127.0.0.1:8080:8080 \
  -e WAYNUDGE_USERS -v waynudge-data:/data waynudge
```

이미지는 UID 10001로 실행합니다. 호스트 디렉터리를 바인드 마운트한다면 해당 UID에 쓰기 권한을 주어야 합니다. `scripts/tunnel.sh`를 호스트에서 실행하면 이 컨테이너의 로컬 포트를 연결합니다. 비공개 GHCR 이미지는 해당 저장소의 packages 읽기 권한으로 로그인해야 내려받을 수 있습니다.

## 고정 공개 MCP와 실시간 조회

현재 주소는 `https://waynudge.helloworld0822.xyz/mcp`입니다. 링크만으로 연결하여 `get_subway_arrivals`, `get_bus_arrivals`, `get_subway_congestion`을 사용할 수 있습니다. 익명 루틴 목록은 비어 있고 개인 데이터 저장·수정에는 인증이 필요합니다. 서버의 API 키는 공개 응답에 포함하지 않습니다.

curl 접속은 검증했으며 일부 클라이언트 서명(Python urllib 기본값)은 Cloudflare Error 1010으로 차단되었습니다. 이 경우 서버 인증 오류가 아니라 Cloudflare 정책입니다. 해당 호스트의 `/mcp`에 한정한 [Browser Integrity Check 예외 설정](https://developers.cloudflare.com/waf/tools/browser-integrity-check/#disable-selectively)이 필요할 수 있습니다.

```json
{"name":"get_subway_arrivals","arguments":{"station_name":"서울","line_id":"1001","direction":"up"}}
```

```json
{"name":"get_bus_arrivals","arguments":{"provider":"tago","station_id":"DJB8001793","city_code":25}}
```

```json
{"name":"get_bus_arrivals","arguments":{"provider":"gyeonggi","station_id":"200000078"}}
```

버스 정류소 ID는 제공자별로 다르므로 지도 서비스 ID를 그대로 바꿔 넣지 않습니다. 경기도 응답의 `crowding`은 여유·보통·혼잡·매우혼잡을 나타내고, `remaining_seats`는 좌석 제공 노선의 잔여좌석입니다. 미제공·음수 값은 `null`이며 여유나 0명으로 해석하지 않습니다. TAGO 도착 응답과 서울시 지하철 도착 응답에 없는 혼잡도를 만들어내지 않습니다. SK API는 사용하지 않습니다. [경기도 API 필드 설명](https://www.gbis.go.kr/gbis2014/publicService.action?cmd=mBusArrivalStation)

### 지하철 평균 혼잡도

[서울교통공사 지하철혼잡도정보](https://www.data.go.kr/data/15071311/fileData.do)의 Open API를 사용합니다. 별도 활용신청 후 `.env`의 `SUBWAY_CONGESTION_SERVICE_KEY`에 Encoding 또는 Decoding 키를 저장합니다. 버스 키에 해당 API 권한이 자동으로 생기지 않습니다. `SUBWAY_CONGESTION_DAILY_LIMIT` 기본값은 로컬 제한 100회이며 실제 발급 한도 이내로 설정합니다.

```json
{"name":"get_subway_congestion","arguments":{"station_name":"서울역","line":1,"day_type":"weekday","time":"08:45","direction":"up"}}
```

`day_type`은 `weekday`, `saturday`, `sunday`이며 시간은 Asia/Seoul입니다. 호선은 1~8, 방향은 `up`(상선·내선), `down`(하선·외선)입니다. 방향을 생략하면 양방향을 반환합니다. 공식 데이터의 역명과 방향을 사용하고 지원하지 않는 호선·역에는 값을 추정하지 않습니다.

이 API는 **실시간이 아닌 30분 평균 통계**입니다. 현재 어댑터는 공식 2026-06-30 자료에 고정되어 있으며 `dataset_as_of`, `realtime=false`, 시간 구간, 정원 대비 혼잡도(%)를 반환합니다. 08:45 조회는 08:30~09:00 평균입니다. 100%를 넘을 수 있으며 34%는 좌석이 찬 수준이라는 공급자 정의입니다. 결측값은 `null`입니다. 조회 시각 `served_at`을 데이터 측정 시각으로 설명하지 않습니다.

Tokio/Reqwest 비동기 호출, 8초 HTTP·10초 전체 타임아웃, 1시간 캐시, 4MiB 응답 제한을 적용합니다. 2,000행까지 한 번에 수집하며 응답이 잘리면 오류를 반환합니다. 분기 자료 변경 시 `src/providers/congestion.rs`의 공식 endpoint와 기준일을 함께 갱신해야 합니다. 통계를 실시간 지연이나 열차별 탑승 가능 여부로 변환하지 않습니다.

서울시 실시간 지하철 도착 API는 HTTP 주소를 사용합니다. 이 환경에서 HTTPS 연결은 실패했으며, 해당 제공자까지의 구간은 TLS로 암호화되지 않는 제약이 있습니다. 인증키·요청 URL을 로그에 기록하지 않으며 API 오류·오래된 관측·알 수 없는 0초 값을 정상 실시간 성공으로 바꾸지 않습니다. 버스와 평균 혼잡도 API는 HTTPS를 사용합니다. 각 실시간 어댑터는 Tokio/Reqwest 비동기, 8초 요청·10초 전체 타임아웃, 15초 캐시, 일별 상한을 적용합니다.

### 별도 혼잡도 시뮬레이션

`simulate_congestion`은 API 키 없이 임의 혼잡도(0~400%, 최대 10개)를 넣는 테스트 전용 도구입니다. `get_subway_congestion`의 공식 통계 응답, 저장 루틴, 경로 선택과 실제 지연값을 변경하지 않습니다. 모든 응답은 `source=simulation`, `realtime=false`, `validated_prediction=false`입니다. 지연 확률을 계산하지 않습니다.

```bash
python3 -c 'import json; print(json.dumps({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"simulate_congestion","arguments":json.load(open("examples/simulation.json"))}}))' | \
  curl --fail-with-body -H 'Content-Type: application/json' \
    -H 'Accept: application/json, text/event-stream' --data-binary @- \
    https://waynudge.helloworld0822.xyz/mcp
```

가정 정책 기본값은 혼잡도 100%를 초과한 100%p마다 역내 도보시간 25%와 정차역당 10초를 추가하는 것입니다. **실측으로 학습하거나 검증한 계수가 아니며 테스트용 여유시간 정책**입니다. `context.policy`로 임계값·도보 비율·정차당 초를 명시해 변경할 수 있고 실제 지연 예측으로 표시하지 않습니다.

`base_trip_minutes`는 역내 도보를 포함한 기본 이동시간, `station_walk_minutes`는 혼잡 영향을 가정할 역내 도보·환승 시간, `remaining_train_stops`는 남은 정차 횟수입니다. `walking_factor`는 도보 부분에 한 번 적용합니다. 실시간 지연을 이미 포함한 시간에 같은 원인의 여유시간을 중복해서 더하지 마세요. 열차 혼잡률은 보행 밀도가 아니므로 두 값을 연결하는 것 자체도 이 테스트의 가정입니다.

기본 40분·역내 도보 6분·정차 10회에 혼잡도 150%를 넣으면 도보 여유 45초와 정차 여유 50초, 총 95초가 추가되어 계산상 41분 35초가 됩니다. `depart_at`과 `context.target_arrival`이 있으면 도착 시각과 목표 도착 가능 여부를 계산합니다. `context.buffer_minutes`는 권장 출발 시각에서 별도로 차감합니다.

`boarding`은 다음 열차까지 남은 초와 승강장까지 걷는 분을 사용해 해당 가정에서 탈 수 있는지 비교합니다. 열차를 놓치는 경우 다음 열차 간격이 없으므로 도착 시각·목표 도착 여부·도보 대비 유불리는 `null`입니다. `walking_alternative_minutes`는 전체 구간을 걸을 때의 기본 소요시간이며 개인 도보 속도를 적용해 도보 도착 가능 여부를 별도로 계산합니다. 확률이나 실제 탑승 보장을 뜻하지 않습니다.

## 모델 에이전트 예약 알림

“평일 오전 7시 30분마다 공휴일 제외하고 출근 알림을 보내줘”라고 `schedule-commute` Skill에 요청합니다. `prepare_commute_reminder`가 검증한 Asia/Seoul 반복 일정과 프롬프트를 만들고 호스트 예약 도구가 실제로 등록합니다. 반환된 `created=false`는 아직 호스트에 등록되지 않았다는 의미입니다.

알림에는 `prepare_at`, `leave_at`, `boarding.station_name`, `boarding.planned_board_at`을 사용해 준비·출발·승차 시각을 표시합니다. `next_observed_train_at`은 계획 시각과 구분된 관측값이며 열차가 목적지까지 운행하는지 별도로 확인해야 합니다. 공휴일 제외는 프롬프트에 포함되며 서버 공휴일 달력을 사용하지 않습니다.

현재 고정 도메인은 사용자 systemd 서비스 `waynudge.service`, `waynudge-tunnel.service`로 실행됩니다. 비밀 키는 프로젝트의 권한600 `.env`에서 읽고, DB는 `~/.local/share/waynudge/waynudge.db`에 저장합니다. 사용자 세션 종료 후에도 실행하도록 linger를 활성화했습니다. 호스트가 꺼지면 서비스도 중단됩니다.
