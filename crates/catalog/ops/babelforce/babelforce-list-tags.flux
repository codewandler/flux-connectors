op babelforce-list-tags -> Any
  description "List all tags"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/tags")
  response = http.request(method: "GET", url)
  return response
