op babelforce-delete-agent(id: String) -> Any
  description "Delete an agent"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}")
  response = http.request(method: "DELETE", url)
  return response
