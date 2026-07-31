op clickup-task-update(task_id: String, name: String, description: String, status: String, priority: Number, due_date: Number) -> Any
  description "Update a task's plain fields (name, description, status, priority, due date). Every field below is optional and ClickUp's update is sparse: an omitted field is left unchanged. Reassigning or rewatching the task is not supported by this operation — ClickUp takes those as {add, rem} deltas naming specific people, which this connector does not declare"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/task/{task_id}")
  content_type = "application/json"
  payload = { description, due_date, name, priority, status }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
