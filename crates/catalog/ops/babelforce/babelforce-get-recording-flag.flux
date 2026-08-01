op babelforce-get-recording-flag(id: String) -> Any
  description "Get a recording's flag state"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings/{id}/flag")
  response = http.request(method: "GET", url)
  return response
