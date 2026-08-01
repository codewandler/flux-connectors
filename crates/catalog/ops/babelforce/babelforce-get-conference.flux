op babelforce-get-conference(id: String) -> Any
  description "Get Conference"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conferences/{id}")
  response = http.request(method: "GET", url)
  return response
