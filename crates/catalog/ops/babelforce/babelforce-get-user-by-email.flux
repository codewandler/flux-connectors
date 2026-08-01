op babelforce-get-user-by-email(email: String) -> Any
  description "Get a user by email"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/users/by-email/{email}")
  response = http.request(method: "GET", url)
  return response
