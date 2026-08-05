op babelforce-list-agent-presences -> Any
  description "List agent presence states"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/presence/available")
  response = http.request(method: "GET", url)
  return response
