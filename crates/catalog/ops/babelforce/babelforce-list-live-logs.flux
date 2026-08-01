op babelforce-list-live-logs(filters_level: Any) -> Any
  description "List live logs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/logs")
  sep = "?"
  when filters_level
    url = fmt("{url}{sep}filters.level={filters_level}")
  response = http.request(method: "GET", url)
  return response
