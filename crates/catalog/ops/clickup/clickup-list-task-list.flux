op clickup-list-task-list(list_id: String, archived: Bool, page: Number, order_by: String, reverse: Bool, include_closed: Bool) -> Any
  description "List tasks in a list, most recently created first by default"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.clickup.com/api/v2"
  url = fmt("{base}/list/{list_id}/task")
  sep = "?"
  when archived
    url = fmt("{url}{sep}archived={archived}")
    sep = "&"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when order_by
    url = fmt("{url}{sep}order_by={order_by}")
    sep = "&"
  when reverse
    url = fmt("{url}{sep}reverse={reverse}")
    sep = "&"
  when include_closed
    url = fmt("{url}{sep}include_closed={include_closed}")
  response = http.request(method: "GET", url)
  return response
