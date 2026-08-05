op clickup-task-get(task_id: String, include_subtasks: Bool) -> Any
  description "Get one task by id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/task/{task_id}")
  response = http.request(method: "GET", query: { include_subtasks }, url)
  return response
