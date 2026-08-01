op babelforce-list-agent-logs(id: String, page: Number, max: Number, filters_from: Number, filters_to: Number) -> Any
  description "List an agent's activity logs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}/logs")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when max
    url = fmt("{url}{sep}max={max}")
    sep = "&"
  when filters_from
    url = fmt("{url}{sep}filters.from={filters_from}")
    sep = "&"
  when filters_to
    url = fmt("{url}{sep}filters.to={filters_to}")
  response = http.request(method: "GET", url)
  return response
