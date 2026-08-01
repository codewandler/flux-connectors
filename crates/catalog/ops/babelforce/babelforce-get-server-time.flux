op babelforce-get-server-time -> Any
  description "Get the current server time and default timezone"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/data/time")
  response = http.request(method: "GET", url)
  return response
