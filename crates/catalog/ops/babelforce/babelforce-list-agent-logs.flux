op babelforce-list-agent-logs(id: String, page: Number, max: Number, filters_from: Number, filters_to: Number) -> Any
  description "List an agent's activity logs"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}/logs")
  response = http.request(method: "GET", query: { "filters.from": filters_from, "filters.to": filters_to, max, page }, url)
  return response
