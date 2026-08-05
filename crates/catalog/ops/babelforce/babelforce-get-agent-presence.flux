op babelforce-get-agent-presence(presenceName: String) -> Any
  description "Get a presence"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/presence/available/{presenceName}")
  response = http.request(method: "GET", url)
  return response
