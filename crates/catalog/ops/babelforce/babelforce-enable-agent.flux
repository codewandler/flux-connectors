op babelforce-enable-agent(id: String) -> Any
  description "Enable an agent"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}/enable")
  response = http.request(method: "PUT", url)
  return response
