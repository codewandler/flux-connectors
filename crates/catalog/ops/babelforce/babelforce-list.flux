op babelforce-list(filter: String, type: String, details: String, page_size: Number, page: Number, context: Bool) -> Any
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/logs/customer")
  response = http.request(method: "GET", query: { context, details, filter, page, page_size, type }, url)
  return response
