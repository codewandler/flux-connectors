op babelforce-get-dialer-info -> Any
  description "Get inbound dialer runtime information"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dialer")
  response = http.request(method: "GET", url)
  return response
