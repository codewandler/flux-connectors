op babelforce-list-scripts(type: String, filter: String, page: Number, page_size: Number) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/scripts/{type}")
  response = http.request(method: "GET", query: { filter, page, page_size }, url)
  return response
