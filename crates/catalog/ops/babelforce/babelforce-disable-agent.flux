op babelforce-disable-agent(id: String) -> Any
  description "Disable an agent"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}/disable")
  response = http.request(method: "PUT", url)
  return response
