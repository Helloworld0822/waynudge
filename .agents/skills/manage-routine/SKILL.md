---
name: manage-routine
description: WayNudge 출퇴근 루틴의 등록·수정, 휴가·재택·특정일 출근 설정, 실제 이동 피드백을 관리한다. 저장할 변경을 사용자에게 확인한 뒤 적용한다.
---

# 루틴 관리

항상 `list_routines`를 먼저 호출한다. 기존 ID와 설정을 확인하고 사용자 요청에 해당하는 루틴을 선택한다. 입력에 사용자 ID·인증 토큰을 넣지 않는다. 루틴이 없으면 신규 등록을 진행하고, 여러 개라면 대상이 명확해질 때까지 저장하지 않는다.

등록·수정은 `upsert_routine`의 `routine` 전체 객체를 사용한다. 기존 루틴 수정 시 요청하지 않은 값을 유지한다. 필수 필드는 `id`, `name`, `origin`과 `destination`(각각 `name`, `lat`, `lon`), `arrival_time`(`HH:MM`, Asia/Seoul), `weekdays`(월=1~일=7), `preferred_modes`(`subway`, `bus`, `mixed`, `walk`), `max_transfers`, `max_walk_minutes`, `buffer_minutes`, `preparation_minutes`, `min_alert_interval_minutes`, `walking_factor`, `usual_route_id`다. ID를 임의 변경해 새 루틴을 만들지 않는다. 장소와 좌표는 사용자가 제공한 값을 사용하고 누락된 필수 조건을 묻는다.

기본값 제안이 필요하면 환승 2회, 도보 20분, 여유 10분, 준비 30분, 알림 간격 15분, 도보 배율 1.0, 평소 경로 ID `null`을 제안할 수 있다. 제안은 저장된 값처럼 말하지 않는다. 선호 이동수단은 엄격한 필터이며 `mixed`는 별도 값이다. 요일·좌표·도착 시각·선호 수단을 만들어내지 않는다.

휴가·재택 등 특정일 제외에는 `set_routine_day_status`로 `routine_id`, `date`, `enabled=false`를 보낸다. 특정일 출근에는 `enabled=true`를 사용한다. 날짜 설정은 요일과 서버 공휴일 설정보다 우선한다. 공휴일 달력은 내장되어 있지 않고 운영자가 `HOLIDAYS`를 설정해야 한다. 요청한 날짜만 변경하며 삭제·날짜 예외 초기화 도구가 있다고 가정하지 않는다.

실제 이동 기록은 `record_trip_feedback`으로 `routine_id`, `route_id`, `actual_minutes`, `walking_factor`를 보낸다. 실제 소요 시간은 사용자가 제공해야 한다. 도보 배율을 모르면 `null`로 보내고 추정 계산하지 않는다. 배율을 지정하면 루틴의 도보 배율도 바뀐다는 점을 요약에 포함한다. `actual_minutes`는 1~1440분, 배율은 0.5~3.0이다. 경로 ID는 이전 도구 응답이나 사용자 제공 값으로 확인한다.

모든 변경 전 대상·변경값·적용 날짜·피드백의 배율 영향까지 짧게 요약하고 사용자의 명시적인 확인을 받는다. 확인 전 `confirmed=true`를 보내지 않는다. 확인된 값이 달라지면 다시 확인한다. 성공 응답의 `saved=true`를 확인한 뒤 완료를 알린다. 실패하거나 응답이 불명확하면 저장됐다고 말하지 않는다. 피드백 저장은 중복 가능하므로 응답 누락 시 자동 재전송하지 않는다.

출퇴근 시각·순위·최적 경로 계산은 서버 도구에 맡긴다. 경로 조회를 함께 했다면 모의 데이터, 실시간 미지원, 빈 결과, 날짜 제외, API 실패를 숨기지 말고 한국어로 간결히 전달한다. 상시 위치 추적이나 푸시 알림을 약속하지 않는다.
