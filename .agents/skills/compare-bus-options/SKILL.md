---
name: compare-bus-options
description: WayNudge로 버스 혼잡·잔여 좌석·도로 정체와 성인·청소년·어린이 요금을 반영해 이동 후보를 비교한다. 버스 대안이나 연령별 요금 비교 요청에 사용한다.
---

# 버스 상황과 요금으로 경로 비교

계산은 WayNudge MCP의 `compare_travel_options`에 맡긴다. 이 도구는 호출자가 제공한 후보와 관측을 비교하며 교통 API나 실제 할인 요금을 자동 조회하지 않는다. 서버 배포나 Vooy 설정 변경, 루틴 저장, 예약 알림 생성은 이 스킬의 작업에 포함되지 않는다.

## 도구 사용 순서

1. 연결된 WayNudge의 도구 목록에서 `compare_travel_options`와 현재 입력 스키마를 확인한다. 운영 서버에 새 입력 필드가 없으면 업데이트가 필요하다고 안내하고 해당 필드를 제거해 다른 결과를 내지 않는다.
2. 실제 이동 요청에는 사용자가 제공한 출발지·목적지와 호스트 지도 도구가 제공한 버스·지하철·혼합·도보 후보를 사용한다. 좌표, 거리, 환승, 요금이나 현재 위치를 추정하지 않는다. 지도 도구가 없으면 후보 정보를 요청한다. 단순 테스트 요청에는 아래 합성 예제를 바로 사용한다.
3. 사용자가 지정한 `fare_profile=adult|youth|child`를 전달한다. 연령 구분을 말하지 않으면 서버 기본값인 성인을 사용했다고 표시한다. `fares_won`에는 출처와 확인된 구분별 전체 경로 요금을 넣는다. 환승 할인을 직접 합산하거나 임의 할인율을 적용하지 않는다.
4. 확인된 버스 관측과 도로 추가 지연, 사용자가 요구한 `require_bus_seat`·`avoid_crowded_bus`를 입력해 `compare_travel_options`를 호출한다. 시간은 오프셋 포함 ISO 8601이며 서울 시간은 `+09:00`이다.
5. 응답의 선택 경로, 변경 여부와 사유를 읽고 아래 형식으로 안내한다. 도구 밖에서 소요시간이나 지연 확률을 새로 계산하지 않는다.

## 버스 관측과 도로 지연

버스·혼합 후보에만 다음 선택 필드를 넣는다.

- `bus_observation`: `{observation:{source,observed_at,valid_for_seconds},crowding,remaining_seats}`. 유효기간은 1~3600초, 좌석은 0~200 또는 미확인이다. `crowding`은 `available`, `crowded`, `boarding_unavailable`이다. 호출자 관측은 서버가 검증한 실시간 정보로 소개하지 않는다.
- `boarding_unavailable`은 후보를 제외한다. 좌석 0만으로 입석 탑승도 불가능하다고 해석하지 않는다. 착석 요구가 있을 때만 `require_bus_seat=true`, 혼잡 회피 요구가 있을 때만 `avoid_crowded_bus=true`를 쓴다. 혼잡을 임의의 시간 벌점으로 바꾸지 않는다.
- `road_traffic`: 확인한 관측은 `{kind:"observed",observation:{source,observed_at,valid_for_seconds},additional_seconds}`, 가정은 `{kind:"synthetic",additional_seconds}`로 구분한다. 추가 지연은 0~86400초이며 **지도 후보의 `total_minutes`에 아직 포함되지 않은 부분만** 넣는다. 정체가 이미 반영됐거나 추가분을 구분할 수 없으면 생략한다.

관측은 `depart_at`이 관측 시각부터 유효기간 끝까지인 경우만 적용된다. 출발 시각 누락·미래 관측·만료 관측은 서버가 무시한다. `applied_bus_observation`, `applied_road_traffic`, `road_delay_seconds`와 경고로 적용 여부를 확인한다. 다음 버스 대기 시간은 제공되지 않으므로 만들어내지 않는다.

## 요금과 예외 처리

`fares_won={source,adult,youth,child}`에서 각 금액은 선택값이며 0~1000000원이다. 기존 `cost_won`은 성인 호환값이다. 청소년·어린이 요금이 없으면 결과 `cost_won=null`을 **요금 미확인**으로 표시한다. 무료 도보도 해당 구분의 0원 입력이 있을 때만 0원으로 표시한다. `fare_verified=false`이며 요금 구분은 이 비교 요청에만 적용되고 저장 루틴에 반영되지 않는다.

잘못된 요금·지연·관측 입력은 항목과 허용 범위를 안내한다. 입력 오류를 연결 장애로 설명하거나 정상 값으로 몰래 바꾸지 않는다. 도구 연결 실패 시 계산 결과를 만들지 않는다. 후보가 모두 제외되면 가능한 경로가 없다고 단정하지 말고, 제공된 후보 중 조건에 맞는 경로가 없다고 설명한다.

## 테스트 예제

“청소년 기준으로 버스 정체 10분을 가정해 도보와 비교해줘”에는 다음 합성 입력을 쓸 수 있다. 장소·요금·시간은 모두 테스트 가정이다. 지하철·혼합까지 비교하는 합성 예제는 저장소의 [bus-traffic.json](../../../examples/bus-traffic.json)을 사용한다.

```json
{
  "origin": {"name": "Synthetic origin", "lat": 37.5, "lon": 127.0},
  "destination": {"name": "Synthetic destination", "lat": 37.6, "lon": 127.1},
  "data_source": "mock",
  "fare_profile": "youth",
  "depart_at": "2026-10-25T08:00:00+09:00",
  "congestion_percentages": [100],
  "options": [
    {
      "id": "bus", "label": "Synthetic bus", "mode": "bus",
      "total_minutes": 20, "walk_minutes": 2,
      "road_traffic": {"kind": "synthetic", "additional_seconds": 600},
      "fares_won": {"source": "synthetic tariff", "adult": 1500, "youth": 900, "child": 500}
    },
    {
      "id": "walk", "label": "Synthetic walk", "mode": "walk",
      "total_minutes": 25, "walk_minutes": 25,
      "fares_won": {"source": "synthetic tariff", "adult": 0, "youth": 0, "child": 0}
    }
  ]
}
```

## 사용자 응답

첫 줄에 가정 비교 여부와 적용한 요금 구분을 밝힌다. 후보별 전체 소요시간·도보시간·추가 도로 지연·해당 구분 요금·탑승 조건을 짧은 표로 표시한다. 제외 후보는 경고의 제외 이유로 설명한다. `selected_option_id`, `should_switch`, `reason`에 따라 추천하며, 출발·목표 시각이 있을 때만 응답의 도착·목표 충족 여부를 안내한다.

`source=simulation`, `realtime=false`, `validated_prediction=false`, `fare_verified=false`를 유지한다. 합성 요금을 실제 요금으로, 가상 지연을 실제 교통 상황이나 검증된 지연 확률로 표현하지 않는다.
