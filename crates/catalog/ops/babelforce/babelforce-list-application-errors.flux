op babelforce-list-application-errors -> Any
  description "List application errors"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/errors")
  response = http.request(method: "GET", url)
  return response
