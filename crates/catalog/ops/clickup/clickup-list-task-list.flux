op clickup-list-task-list(list_id: String, archived: Bool, page: Number, order_by: String, reverse: Bool, include_closed: Bool) -> Any
  description "List tasks in a list, most recently created first by default"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/list/{list_id}/task")
  response = http.request(method: "GET", query: { archived, include_closed, order_by, page, reverse }, url)
  return response
