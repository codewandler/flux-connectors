op babelforce-list-actions(type: String) -> Any
  description "List available actions"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/actions")
  sep = "?"
  when type
    url = fmt("{url}{sep}type={type}")
  response = http.request(method: "GET", url)
  return response
