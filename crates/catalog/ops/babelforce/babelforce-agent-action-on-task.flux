op babelforce-agent-action-on-task(taskId: String, agentAction: String, reason: String) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/agent/tasks/{taskId}/{agentAction}")
  content_type = "application/json"
  payload = { reason }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
