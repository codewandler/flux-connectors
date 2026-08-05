op babelforce-get-agent-group(id: String) -> Any
  description "Get an agent group"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/groups/{id}")
  response = http.request(method: "GET", url)
  return response
