op babelforce-get-sms(id: String) -> Any
  description "Get an SMS"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/sms/{id}")
  response = http.request(method: "GET", url)
  return response
