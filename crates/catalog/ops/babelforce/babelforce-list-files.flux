op babelforce-list-files(page: Number, max: Number, sort: String, order: String, type: String, state: String, filename: String, q: String) -> Any
  description "List files"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files")
  response = http.request(method: "GET", query: { filename, max, order, page, q, sort, state, type }, url)
  return response
