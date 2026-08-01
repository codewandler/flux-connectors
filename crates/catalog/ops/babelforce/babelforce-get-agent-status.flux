op babelforce-get-agent-status(id: String) -> Any
  description "Get an agent's status"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}/status")
  response = http.request(method: "GET", url)
  return response
