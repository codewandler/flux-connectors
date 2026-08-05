op babelforce-list-live-logs(filters_level: String) -> Any
  description "List live logs"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/logs")
  response = http.request(method: "GET", query: { "filters.level": filters_level }, url)
  return response
