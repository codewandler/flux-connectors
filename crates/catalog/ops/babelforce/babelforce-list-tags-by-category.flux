op babelforce-list-tags-by-category(category: String) -> Any
  description "List tags filtered by category"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/tags/{category}")
  response = http.request(method: "GET", url)
  return response
