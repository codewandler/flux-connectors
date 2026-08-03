op babelforce-list-all-simple-reporting-calls(page: Number, max: Number, sessionId: String, conversationId: String, id: String, parentId: String, type: String, from: String, fromNumber: String, to: String, toNumber: String, time_start: Number, time_end: Number, agentId: String, queueName: String, agentName: String, bridged: Bool, duration_start: Number, duration_end: Number, waitTime_start: Number, waitTime_end: Number, queueWaitTime_start: Number, queueWaitTime_end: Number, bridgeTime_start: Number, bridgeTime_end: Number, talkTime_start: Number, talkTime_end: Number, holdTime_start: Number, holdTime_end: Number, wrapupTime_start: Number, wrapupTime_end: Number, handleTime_start: Number, handleTime_end: Number) -> Any
  description "List reporting calls (simple)"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting/simple")
  response = http.request(method: "GET", query: { agentId, agentName, "bridgeTime.end": bridgeTime_end, "bridgeTime.start": bridgeTime_start, bridged, conversationId, "duration.end": duration_end, "duration.start": duration_start, from, fromNumber, "handleTime.end": handleTime_end, "handleTime.start": handleTime_start, "holdTime.end": holdTime_end, "holdTime.start": holdTime_start, id, max, page, parentId, queueName, "queueWaitTime.end": queueWaitTime_end, "queueWaitTime.start": queueWaitTime_start, sessionId, "talkTime.end": talkTime_end, "talkTime.start": talkTime_start, "time.end": time_end, "time.start": time_start, to, toNumber, type, "waitTime.end": waitTime_end, "waitTime.start": waitTime_start, "wrapupTime.end": wrapupTime_end, "wrapupTime.start": wrapupTime_start }, url)
  return response
