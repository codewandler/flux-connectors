op babelforce-delete-agent-presence(presenceName: String) -> Any
  description "Delete a presence"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/presence/available/{presenceName}")
  response = http.request(method: "DELETE", url)
  return response
