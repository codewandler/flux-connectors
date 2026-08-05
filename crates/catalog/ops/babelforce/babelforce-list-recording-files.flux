op babelforce-list-recording-files -> Any
  description "List recording files"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/recordings")
  response = http.request(method: "GET", url)
  return response
