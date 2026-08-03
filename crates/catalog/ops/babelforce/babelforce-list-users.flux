op babelforce-list-users(email: String) -> Any
  description "List users"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/users")
  response = http.request(method: "GET", query: { email }, url)
  return response
