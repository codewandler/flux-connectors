op babelforce-agent-interaction-duration(agentId: String) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/metrics/agent/{agentId}/interactions")
  response = http.request(method: "GET", url)
  return response
