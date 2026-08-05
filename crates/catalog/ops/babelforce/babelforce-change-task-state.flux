op babelforce-change-task-state(taskId: String, taskState: String) -> Any
  description "this endpoint is deprecated, use /api/v3/tasks/{taskId}/interrupt/{interruptTo} instead."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/{taskId}/{taskState}")
  response = http.request(method: "PUT", url)
  return response
