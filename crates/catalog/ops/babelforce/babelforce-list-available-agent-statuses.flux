op babelforce-list-available-agent-statuses -> Any
  description "List available agent statuses"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/status/available")
  response = http.request(method: "GET", url)
  return response
