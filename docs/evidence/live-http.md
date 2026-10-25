# Live HTTP and Quick Tunnel evidence

Captured 2026-10-25. Public example coordinates and disposable QA principals only. Initial jq invocation failed because jq is absent; corrected direct JSON upsert and dependent checks succeeded below.

Final scenario: `TRANSIT_PROVIDER=mock MOCK_SUBWAY_DELAY_MINUTES=25` with `current_route_id=mock-subway` returned HTTP200, `should_switch=true`, `selected.id=mock-mixed`, `source=mock`, every route `realtime=false`, and an explicit synthetic delay warning. Full response:

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 1951
date: Sun, 25 Oct 2026 03:32:07 GMT

{"routine_id":"morning-commute","date":"2026-10-26","active":true,"timezone":"Asia/Seoul","target_arrival":"2026-10-26T09:00:00+09:00","leave_at":"2026-10-26T08:15:00+09:00","prepare_at":"2026-10-26T07:45:00+09:00","estimated_arrival_if_leaving_now":"2026-10-25T13:07:07.391766687+09:00","late_if_leaving_now":false,"comparison":{"selected":{"id":"mock-mixed","mode":"mixed","total_minutes":35,"walk_minutes":10,"transfers":2,"cost_won":1500,"collected_at":"2026-10-25T03:32:07.393597296Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}},"candidates":[{"id":"mock-mixed","mode":"mixed","total_minutes":35,"walk_minutes":10,"transfers":2,"cost_won":1500,"collected_at":"2026-10-25T03:32:07.393597296Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}},{"id":"mock-bus","mode":"bus","total_minutes":44,"walk_minutes":4,"transfers":0,"cost_won":1500,"collected_at":"2026-10-25T03:32:07.393596505Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}},{"id":"mock-subway","mode":"subway","total_minutes":38,"walk_minutes":8,"transfers":1,"cost_won":1500,"collected_at":"2026-10-25T03:32:07.393592627Z","realtime":false,"delay_minutes":25,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}}],"should_switch":true,"reason":"Estimated saving exceeds five minutes plus switching burden.","warnings":["Real-time status unavailable; scheduled estimates only.","Simulated subway delay: 25 minutes; not an observed disruption.","Synthetic demo routes; not real transit or live information."],"source":"mock"}}
```

## Confirmed example routine upsert

```http
/bin/bash: line 1: jq: command not found
HTTP/1.1 400 Bad Request
content-type: text/plain; charset=utf-8
content-length: 86
date: Sun, 25 Oct 2026 03:25:50 GMT

Failed to parse the request body as JSON: EOF while parsing a value at line 1 column 0
```

## Owner routines

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' http://127.0.0.1:8080/api/routines`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 15
date: Sun, 25 Oct 2026 03:25:50 GMT

{"routines":[]}
```

## Other user sees empty routines

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' http://127.0.0.1:8080/api/routines`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 15
date: Sun, 25 Oct 2026 03:25:50 GMT

{"routines":[]}
```

## Seoul departure brief

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' --data '{"routine_id":"morning-commute","date":"2026-10-26","current_route_id":"mock-subway"}' http://127.0.0.1:8080/api/brief`

```http
HTTP/1.1 404 Not Found
content-type: application/json
content-length: 29
date: Sun, 25 Oct 2026 03:25:50 GMT

{"error":"routine not found"}
```

## MCP initialize

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"live-qa","version":"1"}}}' http://127.0.0.1:8080/mcp`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 141
date: Sun, 25 Oct 2026 03:25:50 GMT

{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"rmcp","version":"3.7.0"}}}
```

## MCP tools/list

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' http://127.0.0.1:8080/mcp`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 12758
date: Sun, 25 Oct 2026 03:25:50 GMT

{"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"compare_routes","description":"Compare available transit routes for a commute routine.","inputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"current_route_id":{"type":["string","null"]},"date":{"type":"string"},"routine_id":{"type":"string"}},"required":["routine_id","date"],"type":"object"},"outputSchema":{"$defs":{"Mode":{"enum":["subway","bus","mixed","walk"],"type":"string"},"Place":{"additionalProperties":false,"properties":{"lat":{"format":"double","type":"number"},"lon":{"format":"double","type":"number"},"name":{"type":"string"}},"required":["name","lat","lon"],"type":"object"},"Route":{"properties":{"collected_at":{"format":"date-time","type":"string"},"cost_won":{"format":"uint32","minimum":0,"type":"integer"},"delay_minutes":{"format":"uint32","minimum":0,"type":"integer"},"destination":{"$ref":"#/$defs/Place"},"id":{"type":"string"},"mode":{"$ref":"#/$defs/Mode"},"origin":{"$ref":"#/$defs/Place"},"realtime":{"type":"boolean"},"total_minutes":{"format":"uint32","minimum":0,"type":"integer"},"transfers":{"format":"uint32","minimum":0,"type":"integer"},"walk_minutes":{"format":"uint32","minimum":0,"type":"integer"}},"required":["id","mode","total_minutes","walk_minutes","transfers","cost_won","collected_at","realtime","delay_minutes","origin","destination"],"type":"object"}},"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"candidates":{"items":{"$ref":"#/$defs/Route"},"type":"array"},"reason":{"type":"string"},"selected":{"anyOf":[{"$ref":"#/$defs/Route"},{"type":"null"}]},"should_switch":{"type":"boolean"},"source":{"type":"string"},"warnings":{"items":{"type":"string"},"type":"array"}},"required":["candidates","should_switch","reason","warnings","source"],"title":"Comparison","type":"object"},"annotations":{"readOnlyHint":true}},{"name":"get_commute_brief","description":"Get a departure decision and route summary for a commute routine.","inputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"current_route_id":{"type":["string","null"]},"date":{"type":"string"},"routine_id":{"type":"string"}},"required":["routine_id","date"],"type":"object"},"outputSchema":{"$defs":{"Comparison":{"properties":{"candidates":{"items":{"$ref":"#/$defs/Route"},"type":"array"},"reason":{"type":"string"},"selected":{"anyOf":[{"$ref":"#/$defs/Route"},{"type":"null"}]},"should_switch":{"type":"boolean"},"source":{"type":"string"},"warnings":{"items":{"type":"string"},"type":"array"}},"required":["candidates","should_switch","reason","warnings","source"],"type":"object"},"Mode":{"enum":["subway","bus","mixed","walk"],"type":"string"},"Place":{"additionalProperties":false,"properties":{"lat":{"format":"double","type":"number"},"lon":{"format":"double","type":"number"},"name":{"type":"string"}},"required":["name","lat","lon"],"type":"object"},"Route":{"properties":{"collected_at":{"format":"date-time","type":"string"},"cost_won":{"format":"uint32","minimum":0,"type":"integer"},"delay_minutes":{"format":"uint32","minimum":0,"type":"integer"},"destination":{"$ref":"#/$defs/Place"},"id":{"type":"string"},"mode":{"$ref":"#/$defs/Mode"},"origin":{"$ref":"#/$defs/Place"},"realtime":{"type":"boolean"},"total_minutes":{"format":"uint32","minimum":0,"type":"integer"},"transfers":{"format":"uint32","minimum":0,"type":"integer"},"walk_minutes":{"format":"uint32","minimum":0,"type":"integer"}},"required":["id","mode","total_minutes","walk_minutes","transfers","cost_won","collected_at","realtime","delay_minutes","origin","destination"],"type":"object"}},"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"active":{"type":"boolean"},"comparison":{"$ref":"#/$defs/Comparison"},"date":{"type":"string"},"estimated_arrival_if_leaving_now":{"type":["string","null"]},"late_if_leaving_now":{"type":"boolean"},"leave_at":{"type":["string","null"]},"prepare_at":{"type":["string","null"]},"routine_id":{"type":"string"},"target_arrival":{"type":"string"},"timezone":{"type":"string"}},"required":["routine_id","date","active","timezone","target_arrival","late_if_leaving_now","comparison"],"title":"Brief","type":"object"},"annotations":{"readOnlyHint":true}},{"name":"list_routines","description":"List the authenticated user's commute routines.","inputSchema":{"properties":{},"type":"object"},"outputSchema":{"$defs":{"Mode":{"enum":["subway","bus","mixed","walk"],"type":"string"},"Place":{"additionalProperties":false,"properties":{"lat":{"format":"double","type":"number"},"lon":{"format":"double","type":"number"},"name":{"type":"string"}},"required":["name","lat","lon"],"type":"object"},"Routine":{"additionalProperties":false,"properties":{"arrival_time":{"description":"Local arrival time, HH:MM, Asia/Seoul.","type":"string"},"buffer_minutes":{"format":"uint32","minimum":0,"type":"integer"},"destination":{"$ref":"#/$defs/Place"},"id":{"type":"string"},"max_transfers":{"format":"uint32","minimum":0,"type":"integer"},"max_walk_minutes":{"format":"uint32","minimum":0,"type":"integer"},"min_alert_interval_minutes":{"format":"uint32","minimum":0,"type":"integer"},"name":{"type":"string"},"origin":{"$ref":"#/$defs/Place"},"preferred_modes":{"items":{"$ref":"#/$defs/Mode"},"type":"array"},"preparation_minutes":{"format":"uint32","minimum":0,"type":"integer"},"usual_route_id":{"type":["string","null"]},"walking_factor":{"description":"Walking duration multiplier; 1.2 means 20% slower than provider baseline.","format":"double","type":"number"},"weekdays":{"description":"Monday=1 through Sunday=7.","items":{"format":"uint32","minimum":0,"type":"integer"},"type":"array"}},"required":["id","name","origin","destination","arrival_time","weekdays","preferred_modes","max_transfers","max_walk_minutes","buffer_minutes","preparation_minutes","min_alert_interval_minutes","walking_factor"],"type":"object"}},"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"routines":{"items":{"$ref":"#/$defs/Routine"},"type":"array"}},"required":["routines"],"title":"RoutineList","type":"object"},"annotations":{"readOnlyHint":true}},{"name":"record_trip_feedback","description":"Record observed trip duration and walking feedback after explicit user confirmation.","inputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"actual_minutes":{"format":"uint32","minimum":0,"type":"integer"},"confirmed":{"type":"boolean"},"route_id":{"type":"string"},"routine_id":{"type":"string"},"walking_factor":{"format":"double","type":["number","null"]}},"required":["routine_id","route_id","actual_minutes","confirmed"],"type":"object"},"outputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"saved":{"type":"boolean"}},"required":["saved"],"title":"MutationResult","type":"object"},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":false}},{"name":"replan_trip","description":"Replan a commute from its current trip context.","inputSchema":{"$defs":{"Place":{"additionalProperties":false,"properties":{"lat":{"format":"double","type":"number"},"lon":{"format":"double","type":"number"},"name":{"type":"string"}},"required":["name","lat","lon"],"type":"object"},"TripStage":{"enum":["at_station","walking","transfer_missed"],"type":"string"}},"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"blocked_route_ids":{"items":{"type":"string"},"type":"array"},"current_location":{"$ref":"#/$defs/Place","description":"User-provided current station/location; no continuous tracking."},"current_route_id":{"type":["string","null"]},"date":{"type":"string"},"routine_id":{"type":"string"},"stage":{"$ref":"#/$defs/TripStage"}},"required":["routine_id","date","current_location","stage","blocked_route_ids"],"type":"object"},"outputSchema":{"$defs":{"Comparison":{"properties":{"candidates":{"items":{"$ref":"#/$defs/Route"},"type":"array"},"reason":{"type":"string"},"selected":{"anyOf":[{"$ref":"#/$defs/Route"},{"type":"null"}]},"should_switch":{"type":"boolean"},"source":{"type":"string"},"warnings":{"items":{"type":"string"},"type":"array"}},"required":["candidates","should_switch","reason","warnings","source"],"type":"object"},"Mode":{"enum":["subway","bus","mixed","walk"],"type":"string"},"Place":{"additionalProperties":false,"properties":{"lat":{"format":"double","type":"number"},"lon":{"format":"double","type":"number"},"name":{"type":"string"}},"required":["name","lat","lon"],"type":"object"},"Route":{"properties":{"collected_at":{"format":"date-time","type":"string"},"cost_won":{"format":"uint32","minimum":0,"type":"integer"},"delay_minutes":{"format":"uint32","minimum":0,"type":"integer"},"destination":{"$ref":"#/$defs/Place"},"id":{"type":"string"},"mode":{"$ref":"#/$defs/Mode"},"origin":{"$ref":"#/$defs/Place"},"realtime":{"type":"boolean"},"total_minutes":{"format":"uint32","minimum":0,"type":"integer"},"transfers":{"format":"uint32","minimum":0,"type":"integer"},"walk_minutes":{"format":"uint32","minimum":0,"type":"integer"}},"required":["id","mode","total_minutes","walk_minutes","transfers","cost_won","collected_at","realtime","delay_minutes","origin","destination"],"type":"object"}},"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"active":{"type":"boolean"},"comparison":{"$ref":"#/$defs/Comparison"},"date":{"type":"string"},"estimated_arrival_if_leaving_now":{"type":["string","null"]},"late_if_leaving_now":{"type":"boolean"},"leave_at":{"type":["string","null"]},"prepare_at":{"type":["string","null"]},"routine_id":{"type":"string"},"target_arrival":{"type":"string"},"timezone":{"type":"string"}},"required":["routine_id","date","active","timezone","target_arrival","late_if_leaving_now","comparison"],"title":"Brief","type":"object"},"annotations":{"readOnlyHint":true}},{"name":"set_routine_day_status","description":"Enable or skip a routine on a date after explicit user confirmation.","inputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"confirmed":{"type":"boolean"},"date":{"type":"string"},"enabled":{"type":"boolean"},"routine_id":{"type":"string"}},"required":["routine_id","date","enabled","confirmed"],"type":"object"},"outputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"saved":{"type":"boolean"}},"required":["saved"],"title":"MutationResult","type":"object"},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true}},{"name":"upsert_routine","description":"Create or update a commute routine after explicit user confirmation.","inputSchema":{"$defs":{"Mode":{"enum":["subway","bus","mixed","walk"],"type":"string"},"Place":{"additionalProperties":false,"properties":{"lat":{"format":"double","type":"number"},"lon":{"format":"double","type":"number"},"name":{"type":"string"}},"required":["name","lat","lon"],"type":"object"},"Routine":{"additionalProperties":false,"properties":{"arrival_time":{"description":"Local arrival time, HH:MM, Asia/Seoul.","type":"string"},"buffer_minutes":{"format":"uint32","minimum":0,"type":"integer"},"destination":{"$ref":"#/$defs/Place"},"id":{"type":"string"},"max_transfers":{"format":"uint32","minimum":0,"type":"integer"},"max_walk_minutes":{"format":"uint32","minimum":0,"type":"integer"},"min_alert_interval_minutes":{"format":"uint32","minimum":0,"type":"integer"},"name":{"type":"string"},"origin":{"$ref":"#/$defs/Place"},"preferred_modes":{"items":{"$ref":"#/$defs/Mode"},"type":"array"},"preparation_minutes":{"format":"uint32","minimum":0,"type":"integer"},"usual_route_id":{"type":["string","null"]},"walking_factor":{"description":"Walking duration multiplier; 1.2 means 20% slower than provider baseline.","format":"double","type":"number"},"weekdays":{"description":"Monday=1 through Sunday=7.","items":{"format":"uint32","minimum":0,"type":"integer"},"type":"array"}},"required":["id","name","origin","destination","arrival_time","weekdays","preferred_modes","max_transfers","max_walk_minutes","buffer_minutes","preparation_minutes","min_alert_interval_minutes","walking_factor"],"type":"object"}},"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"confirmed":{"type":"boolean"},"routine":{"$ref":"#/$defs/Routine"}},"required":["routine","confirmed"],"type":"object"},"outputSchema":{"$schema":"https://json-schema.org/draft/2020-12/schema","properties":{"saved":{"type":"boolean"}},"required":["saved"],"title":"MutationResult","type":"object"},"annotations":{"readOnlyHint":false,"destructiveHint":false,"idempotentHint":true}}]}}
```

## MCP tools/call owner

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_routines","arguments":{}}}' http://127.0.0.1:8080/mcp`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 142
date: Sun, 25 Oct 2026 03:25:50 GMT

{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"routines\":[]}"}],"structuredContent":{"routines":[]},"isError":false}}
```

## MCP cross-user isolation

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"list_routines","arguments":{}}}' http://127.0.0.1:8080/mcp`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 142
date: Sun, 25 Oct 2026 03:25:50 GMT

{"jsonrpc":"2.0","id":4,"result":{"content":[{"type":"text","text":"{\"routines\":[]}"}],"structuredContent":{"routines":[]},"isError":false}}
```

## Corrected confirmed routine upsert (jq unavailable; generated JSON directly)

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 14
date: Sun, 25 Oct 2026 03:26:11 GMT

{"saved":true}
```

## Owner routines populated

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' http://127.0.0.1:8080/api/routines`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 459
date: Sun, 25 Oct 2026 03:26:11 GMT

{"routines":[{"id":"morning-commute","name":"평일 출근 예시","origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276},"arrival_time":"09:00","weekdays":[1,2,3,4,5,6,7],"preferred_modes":["subway","bus","mixed"],"max_transfers":2,"max_walk_minutes":20,"buffer_minutes":10,"preparation_minutes":30,"min_alert_interval_minutes":15,"walking_factor":1.0,"usual_route_id":null}]}
```

## Other owner empty after save

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' http://127.0.0.1:8080/api/routines`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 15
date: Sun, 25 Oct 2026 03:26:11 GMT

{"routines":[]}
```

## Brief live success

Command (token redacted): `curl -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' --data '{"routine_id":"morning-commute","date":"2026-10-26","current_route_id":"mock-subway"}' http://127.0.0.1:8080/api/brief`

```http
HTTP/1.1 200 OK
content-type: application/json
content-length: 1884
date: Sun, 25 Oct 2026 03:26:11 GMT

{"routine_id":"morning-commute","date":"2026-10-26","active":true,"timezone":"Asia/Seoul","target_arrival":"2026-10-26T09:00:00+09:00","leave_at":"2026-10-26T08:12:00+09:00","prepare_at":"2026-10-26T07:42:00+09:00","estimated_arrival_if_leaving_now":"2026-10-25T13:04:11.973766014+09:00","late_if_leaving_now":false,"comparison":{"selected":{"id":"mock-subway","mode":"subway","total_minutes":38,"walk_minutes":8,"transfers":1,"cost_won":1500,"collected_at":"2026-10-25T03:26:11.977567308Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}},"candidates":[{"id":"mock-mixed","mode":"mixed","total_minutes":35,"walk_minutes":10,"transfers":2,"cost_won":1500,"collected_at":"2026-10-25T03:26:11.977578807Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}},{"id":"mock-subway","mode":"subway","total_minutes":38,"walk_minutes":8,"transfers":1,"cost_won":1500,"collected_at":"2026-10-25T03:26:11.977567308Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}},{"id":"mock-bus","mode":"bus","total_minutes":44,"walk_minutes":4,"transfers":0,"cost_won":1500,"collected_at":"2026-10-25T03:26:11.977576831Z","realtime":false,"delay_minutes":0,"origin":{"name":"서울역 예시","lat":37.5547,"lon":126.9707},"destination":{"name":"강남역 예시","lat":37.4979,"lon":127.0276}}],"should_switch":false,"reason":"Keep the current route; saving does not justify switching.","warnings":["Real-time status unavailable; scheduled estimates only.","Synthetic demo routes; not real transit or live information."],"source":"mock"}}
```

## Tunnel anonymous blocked

Command (token redacted): `curl --max-time 30 -i -sS https://pie-beautiful-refuse-levels.trycloudflare.com/health`

```http
HTTP/2 401
date: Sun, 25 Oct 2026 03:26:12 GMT
content-type: application/json
content-length: 35
cf-ray: a4fe1e4c3a1c0dbb-ICN
cf-cache-status: DYNAMIC
server: cloudflare
www-authenticate: Bearer
x-robots-tag: none
x-robots-tag: none

{"error":"authentication required"}
```

## Tunnel authenticated health

Command (token redacted): `curl --max-time 30 -i -sS -H 'Authorization: Bearer <QA-token>' https://pie-beautiful-refuse-levels.trycloudflare.com/health`

```http
HTTP/2 200
date: Sun, 25 Oct 2026 03:26:12 GMT
content-type: application/json
content-length: 15
cf-ray: a4fe1e4c5fb1ea19-ICN
cf-cache-status: DYNAMIC
server: cloudflare
x-robots-tag: none
x-robots-tag: none

{"status":"ok"}
```

## Tunnel MCP JSON callable

Command (token redacted): `curl --max-time 30 -i -sS -H 'Authorization: Bearer <QA-token>' -H 'Content-Type: application/json' -H 'Accept: application/json, text/event-stream' --data '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_routines","arguments":{}}}' https://pie-beautiful-refuse-levels.trycloudflare.com/mcp`

```http
HTTP/2 200
date: Sun, 25 Oct 2026 03:26:12 GMT
content-type: application/json
content-length: 1086
cf-ray: a4fe1e4c7cc1b5e6-ICN
cf-cache-status: DYNAMIC
server: cloudflare
x-robots-tag: none
x-robots-tag: none

{"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{\"routines\":[{\"arrival_time\":\"09:00\",\"buffer_minutes\":10,\"destination\":{\"lat\":37.4979,\"lon\":127.0276,\"name\":\"강남역 예시\"},\"id\":\"morning-commute\",\"max_transfers\":2,\"max_walk_minutes\":20,\"min_alert_interval_minutes\":15,\"name\":\"평일 출근 예시\",\"origin\":{\"lat\":37.5547,\"lon\":126.9707,\"name\":\"서울역 예시\"},\"preferred_modes\":[\"subway\",\"bus\",\"mixed\"],\"preparation_minutes\":30,\"usual_route_id\":null,\"walking_factor\":1.0,\"weekdays\":[1,2,3,4,5,6,7]}]}"}],"structuredContent":{"routines":[{"arrival_time":"09:00","buffer_minutes":10,"destination":{"lat":37.4979,"lon":127.0276,"name":"강남역 예시"},"id":"morning-commute","max_transfers":2,"max_walk_minutes":20,"min_alert_interval_minutes":15,"name":"평일 출근 예시","origin":{"lat":37.5547,"lon":126.9707,"name":"서울역 예시"},"preferred_modes":["subway","bus","mixed"],"preparation_minutes":30,"usual_route_id":null,"walking_factor":1.0,"weekdays":[1,2,3,4,5,6,7]}]},"isError":false}}
```
