op babelforce-reset-password -> Any
  description "Request a Password Change"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/user/reset-password")
  response = http.request(method: "POST", url)
  return response
