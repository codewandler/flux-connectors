op babelforce-delete-agent-group(id: String) -> Any
  description "Delete an agent group"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/groups/{id}")
  response = http.request(method: "DELETE", url)
  return response
