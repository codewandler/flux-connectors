op babelforce-list-all-agent-logs(page: Number, max: Number) -> Any
  description "List all agents' activity logs"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/logs")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
