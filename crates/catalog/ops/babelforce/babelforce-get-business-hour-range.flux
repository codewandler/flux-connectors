op babelforce-get-business-hour-range(id: String, rangeId: String) -> Any
  description "Get a time range"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/business-hours/{id}/ranges/{rangeId}")
  response = http.request(method: "GET", url)
  return response
