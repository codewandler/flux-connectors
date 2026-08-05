op babelforce-change-agent-lock(lockState: String) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/agent/tasks/locking/{lockState}")
  response = http.request(method: "POST", url)
  return response
