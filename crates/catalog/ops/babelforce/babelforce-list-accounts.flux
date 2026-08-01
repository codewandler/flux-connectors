op babelforce-list-accounts -> Any
  description "Get List of available Accounts"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/user/accounts")
  response = http.request(method: "GET", url)
  return response
