op babelforce-list-files-by-type(type: String) -> Any
  description "List files of a storage type"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/by-type/{type}")
  response = http.request(method: "GET", url)
  return response
