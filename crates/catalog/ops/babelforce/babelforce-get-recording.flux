op babelforce-get-recording(id: String) -> Any
  description "Get a recording"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings/{id}")
  response = http.request(method: "GET", url)
  return response
