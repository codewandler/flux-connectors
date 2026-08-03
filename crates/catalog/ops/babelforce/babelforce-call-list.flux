op babelforce-call-list(page: Number, max: Number, sessionId: String, conversationId: String, id: String, type: String, fromNumber: String, toNumber: String, time_start: Number, time_end: Number, agentId: String, q: String, state: String, finishReason: String) -> Any
  description "List and filter calls from the reporting view"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting")
  response = http.request(method: "GET", query: { agentId, conversationId, finishReason, fromNumber, id, max, page, q, sessionId, state, "time.end": time_end, "time.start": time_start, toNumber, type }, url)
  return response
