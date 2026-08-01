op babelforce-enable-live-logging -> Any
  description "Enable live logging"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/logs/enable")
  response = http.request(method: "POST", url)
  return response
