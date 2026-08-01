op babelforce-get-user-customer -> Any
  description "Get user & account information"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/user/account")
  response = http.request(method: "GET", url)
  return response
