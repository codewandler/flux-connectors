op babelforce-call-list(page: Number, max: Number, agentId: String, time_start: Number, time_end: Number, q: String) -> Any
  description "List and filter calls, in the reporting view."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting")
  response = http.request(method: "GET", query: { agentId, max, page, q, "time.end": time_end, "time.start": time_start }, url)
  return response
