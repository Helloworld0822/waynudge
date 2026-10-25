---
name: recipe
description: WayNudge의 임의 혼잡도 시뮬레이션으로 버스·지하철·혼합·도보 경로를 비교한다. 혼잡도 테스트나 가상 지연 실험 요청에 사용한다.
---

# 혼잡도 테스트 레시피

버스 혼잡·좌석·도로 정체와 성인·청소년·어린이 요금을 함께 비교하는 요청은 [compare-bus-options](../compare-bus-options/SKILL.md)의 절차와 테스트 예제를 사용한다. 단일 지하철 가정 실험은 아래 절차를 유지한다.

사용자가 혼잡도 테스트·시뮬레이션·가상 지연을 요청하면 이 테스트 모드를 먼저 적용한다. 실제 루틴, 저장된 장소, 실제 교통 API를 조회하거나 변경하지 않는다. 사용자가 "임의로", "기본값으로", "그냥 테스트"라고 하면 아래 값을 바로 사용하고 추가로 질문하지 않는다.

## 공통 입력 검사

도구를 찾거나 호출하기 전에 입력 범위를 검사한다. 혼잡도에 -10% 등 0~400% 밖의 값이 하나라도 있으면 도구를 호출하지 말고 허용 범위를 안내한다. 이 경우 연결 장애가 아니므로 서비스가 응답하지 않는다거나 팀에 알렸다고 말하지 않는다. 사용자가 주지 않은 정상 값으로 자동 재시도하지 않는다. 이 검사는 여러 경로 비교와 단일 지하철 테스트 모두에 적용한다.

두 도구 모두 `source=simulation`, `realtime=false`, `validated_prediction=false`이며 가정 계산이다. 여러 경로는 `scenarios[].candidates[]`의 `total_seconds`, `walk_seconds`를, 단일 지하철은 `scenarios[].calculation.trip_seconds_with_buffer`를 표시한다. 서로 다른 응답 필드를 섞지 않는다.

## 여러 경로 비교

버스를 포함해 여러 경로나 현재 위치에서의 대안을 비교하면 `compare_travel_options`를 사용한다. 단일 지하철의 승차 가능 조건만 실험하면 아래 `simulate_congestion`을 유지한다. 두 도구 모두 가정 비교이며 실제 지연·지연 확률이 아니다.

실제 경로 비교 요청은 사용자가 확인한 현재 위치와 목적지를 받는다. 호스트 모델의 지도 경로 도구가 있으면 같은 두 지점 사이의 버스·지하철·혼합·도보 후보를 가져와 `data_source=user_provided_map`으로 전달한다. 제공되지 않은 수단·거리·환승·요금·열차 정차 수는 만들지 않는다. 지도 도구가 없으면 사용자에게 후보를 받아야 하며 새 지도 API 키를 요구하지 않는다. 합성 후보를 실제 경로 대신 제시하지 않는다.

`origin`, `destination`은 이름·좌표이며 현재 위치는 GPS나 대화 배경에서 추정하지 않는다. `options`의 각 후보에는 `id`, `label`, `mode`, `total_minutes`, `walk_minutes`를 보낸다. 확인된 경우 `walk_distance_meters`, `station_walk_minutes`, `station_walk_distance_meters`, `remaining_train_stops`, `transfers`, `cost_won`을 보낸다. 역내 도보는 전체 도보의 일부이고 전체 도보는 전체 시간의 일부이며 중복 합산하지 않는다. 없는 거리는 생략한다. `walk` 후보는 전체 시간이 도보 시간과 같고 열차 정차·역내 도보는 0이다.

`congestion_percentages`에는 요청한 0~400% 가정치를 넣는다. 확인된 보행 프로필 또는 `record_walking_observation` 미리보기의 속도가 있으면 `walking_speed_kmh`로 전달하며 시간만으로 추정하지 않는다. `depart_at`, `target_arrival`, `current_option_id`, `preferred_modes`, `max_walk_minutes`, `max_transfers`는 확인한 조건만 보낸다. 혼잡도는 지하철·혼합 후보의 역내 도보·열차 정차 여유에만 적용되며 버스 도로 지연으로 해석하지 않는다.

버스·혼합 후보에만 선택적으로 `bus_observation={observation:{source,observed_at,valid_for_seconds},crowding,remaining_seats}`를 보낸다. `source`는 호출자가 확인한 출처 이름, `observed_at`은 오프셋 포함 관측 시각, 유효기간은 1~3600초다. `depart_at`을 기준으로 관측 시각부터 유효기간 끝까지 적용하며 출발 시각 누락·미래 관측·만료 자료는 경고와 함께 무시한다. 서버가 검증한 실시간 자료라고 말하지 않는다. `crowding`은 `available`, `crowded`, `boarding_unavailable` 중 하나다. 탑승 불가능 후보는 제외하고 다음 버스 대기 시간을 추정하지 않는다. 잔여 좌석 0은 입석 탑승 불가를 뜻하지 않는다. 사용자가 착석을 요구할 때만 요청의 `require_bus_seat=true`를 사용하고, 혼잡 버스를 피하려 할 때만 `avoid_crowded_bus=true`로 `crowded` 후보를 제외한다. 좌석 수는 0~200이며 양수 좌석과 `boarding_unavailable`을 함께 보내지 않는다.

도로 지연은 `road_traffic={kind:"observed",observation:{source,observed_at,valid_for_seconds},additional_seconds}` 또는 명시적 가정인 `{kind:"synthetic",additional_seconds:600}`을 사용한다. `additional_seconds`는 0~86400이며 **`total_minutes`에 아직 포함되지 않은 추가 지연만** 보낸다. 지도 예상 시간에 이미 포함된 정체를 다시 더하지 않는다. 두 종류를 동시에 보낼 수 없으며 지하철·도보 후보에는 적용하지 않는다. 혼잡도 백분율에서 버스 지연 확률이나 시간을 만들어내지 않는다. 결과의 `road_delay_seconds`, `applied_road_traffic`, `applied_bus_observation`으로 실제 적용 여부·출처를 확인한다.

요금 비교에는 요청별 `fare_profile=adult|youth|child`를 사용한다(생략 시 adult). 각 후보의 `fares_won={source,adult,youth,child}`에는 확인된 원 단위 금액만 보낸다(각 금액 생략 가능, 0~1000000). 예: 테스트에서는 `{source:"synthetic fixture",adult:1500,youth:850,child:450}`처럼 가상값임을 밝힌다. 기존 `cost_won`은 성인 요금의 호환용 대체값만 된다. 청소년·어린이 요금이 없으면 `cost_won=null`이며 성인 요금에 임의 할인율을 곱하지 않는다. `fare_source`와 `fare_verified=false`를 함께 설명하고 연령·자격을 추정하지 않는다. 이 설정은 `compare_travel_options` 요청에만 적용되며 저장 루틴이나 `get_commute_brief`의 공급자 요금을 바꾸지 않는다.

“그냥 테스트”에는 `data_source=mock`과 명시적인 합성 장소로 지하철 30분(도보 8분·600m, 역내 4분·300m, 8정차·1환승), 버스 33분(도보 5분·400m, 0환승), 혼합 32분(도보 7분·500m, 역내 3분·200m, 4정차·1환승), 도보 40분(3,000m) 네 후보와 혼잡도 [80,150,200]을 사용할 수 있다. 위치·시간·거리는 실제 관측이 아닌 테스트 가정이라고 밝힌다.

응답의 각 `scenarios`에서 `candidates`의 `total_seconds`·`walk_seconds`·환승·요금, `selected_option_id`, `should_switch`, `reason`, 경고를 설명한다. 출발·목표 시각이 있을 때만 도착과 목표 충족 여부를 안내한다. 테스트 결과로 실제 경로를 자동 변경하거나 루틴을 저장하지 않는다.

## 단일 지하철 조건 실행

연결된 WayNudge MCP(`https://waynudge.helloworld0822.xyz/mcp`, Vooy에서는 `rmcp`)의 `simulate_congestion`을 호출한다. 서비스 검색이 필요한 환경이면 해당 연결의 최신 도구 목록에서 먼저 찾는다. `get_subway_congestion`은 공식 통계 조회이므로 이 가상 테스트의 대체 도구로 사용하지 않는다.

기본 인수는 다음과 같다. 사용자가 지정한 값이 있으면 그 항목만 바꾼다.

```json
{
  "congestion_percentages": [80, 150, 200],
  "context": {
    "base_trip_minutes": 40,
    "station_walk_minutes": 6,
    "remaining_train_stops": 10,
    "walking_factor": 1.0
  },
  "walking_alternative_minutes": 41
}
```

혼잡도는 0~400%의 가정 값이며 한 번에 최대 10개를 비교한다. 기본 이동시간에는 역내 도보시간이 이미 포함된다. `walking_alternative_minutes`는 전체 구간을 걸었을 때의 기본 소요시간이다. `context.policy`를 생략하면 서버가 응답에 명시하는 테스트 정책을 사용한다. 계산은 Rust 도구에 맡기고 자체 계산으로 결과를 대신하지 않는다.

사용자가 출발 시각과 목표 도착 시각을 주면 서울 시간의 오프셋 포함 ISO 8601로 `depart_at`, `context.target_arrival`에 전달한다. 사용자가 승강장까지 걷는 시간과 다음 열차까지 남은 시간을 주면 `boarding.walk_minutes`, `boarding.train_departure_after_seconds`에 전달한다. 시간과 승강장 조건이 없으면 해당 조건은 생략하고 가능한지 추정하지 않는다.

실제 “A에서 B까지 N분 걸었다”는 관측으로 보행 속도를 저장하려는 요청은 `manage-routine`의 보행 보정 절차로 연결한다. 사용자 측정 또는 호스트 지도 도보 경로의 실제 거리와 시간이 있어야 하며 직선거리나 분 단위 시간만으로 속도를 추정하지 않는다. `record_walking_observation`의 `confirmed=false` 미리보기 후 명시적인 동의·인증으로 본인 루틴에만 저장한다. 이 시뮬레이션의 가상 도보시간을 실제 관측으로 저장하지 않는다.

## 단일 지하철 응답

첫 줄에 "임의 혼잡도를 넣은 테스트 결과입니다"라고 밝힌다. 도구 응답의 혼잡도별 기본 시간, `walking_buffer_seconds`, `train_dwell_buffer_seconds`, `trip_seconds_with_buffer`를 짧은 표로 보여준다. 도보 대안이 있으면 `walking_alternative`의 시간과 비교 결과를 덧붙인다. 출발·도착·승차 조건을 입력한 경우에만 해당 결과도 안내한다.

`source=simulation`, `realtime=false`, `validated_prediction=false`를 지킨다. 도보·정차 여유시간 계수는 검증되지 않은 가정 정책이며 실제 열차 지연이나 지연 확률이 아니다. "지연 확률 70%"처럼 도구에 없는 확률을 만들거나 가정 결과로 실제 경로를 자동 변경하지 않는다.

열차를 놓쳐 도착 시각·목표 도착·대중교통 대비 도보 유불리가 `null`이면 "다음 열차 정보가 없어 알 수 없습니다"라고 안내한다. 도보 자체의 도착 조건이 제공된 경우 그 결과는 별도로 보여준다.

## 실패 처리

도구가 연결되지 않았거나 최신 목록에 없으면 연결 문제를 명시한다. 계산하지 않은 예시 숫자를 실제 실행 결과로 제시하지 않는다. 도구의 입력 검증 오류도 연결 장애와 구분하고, 잘못된 항목과 허용 범위를 안내한다. 테스트 요청으로 루틴 저장, 피드백 기록, 예약 알림 생성, 실제 알림 발송을 하지 않는다. 별도 연락 도구의 실제 성공이 없으면 팀에 알렸다는 등의 행동을 했다고 말하지 않는다.
