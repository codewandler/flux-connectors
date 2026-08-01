op babelforce-get-application(id: String) -> Any
  description "Get an application"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{id}")
  response = http.request(method: "GET", url)
  return response
