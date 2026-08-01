op babelforce-list-users(email: Any) -> Any
  description "List users"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/users")
  sep = "?"
  when email
    url = fmt("{url}{sep}email={email}")
  response = http.request(method: "GET", url)
  return response
