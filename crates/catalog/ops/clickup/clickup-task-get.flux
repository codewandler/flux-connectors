op clickup-task-get(task_id: String, include_subtasks: Bool) -> Any
  description "Get one task by id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/task/{task_id}")
  sep = "?"
  when include_subtasks
    url = fmt("{url}{sep}include_subtasks={include_subtasks}")
  response = http.request(method: "GET", url)
  return response
