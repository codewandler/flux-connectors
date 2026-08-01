op babelforce-get-file(id: String) -> Any
  description "Get a file"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/{id}")
  response = http.request(method: "GET", url)
  return response
