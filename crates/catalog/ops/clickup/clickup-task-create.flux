op clickup-task-create(list_id: String, name: String, description: String, status: String, priority: Number, due_date: Number) -> Any
  description "Create a task in a list. Created with `notify_all` left at its default (false), so nobody is emailed by this call"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/list/{list_id}/task")
  content_type = "application/json"
  payload = { description, due_date, name, priority, status }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
