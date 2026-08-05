op babelforce-hangup-agent-call(id: String) -> Any
  description "Hang up an agent's call"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/agents/{id}/hangup")
  response = http.request(method: "POST", url)
  return response
