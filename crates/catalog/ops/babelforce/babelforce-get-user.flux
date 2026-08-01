op babelforce-get-user(id: String) -> Any
  description "Get a user"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/users/{id}")
  response = http.request(method: "GET", url)
  return response
